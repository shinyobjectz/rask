//! Size limits: a Rust file at most FILE_MAX lines and a crate at most CRATE_MAX lines of its own .rs (a nested
//! crate's files count for it, not its parent). A plan's rows say what a file grows to; together with what's on disk
//! they must stay under both limits.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FILE_MAX: usize = 500;
pub const CRATE_MAX: usize = 2000;

#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub dir: PathBuf,
    /// file path relative to `dir` -> lines
    pub files: BTreeMap<String, usize>,
}

impl Crate {
    pub fn lines(&self) -> usize {
        self.files.values().sum()
    }
}

/// Every crate under `root` with its own .rs files and their line counts
pub fn crates(root: &Path) -> Vec<Crate> {
    let mut manifests = vec![];
    let mut rs = vec![];
    crate::walk::files(root, &mut |p| {
        if p.file_name().is_some_and(|n| n == "Cargo.toml") {
            manifests.push(p.to_path_buf());
        } else if p.extension().is_some_and(|x| x == "rs") {
            rs.push(p.to_path_buf());
        }
    });
    let mut out: Vec<Crate> = manifests
        .iter()
        .filter_map(|m| {
            let name = package_name(&std::fs::read_to_string(m).ok()?)?;
            Some(Crate { name, dir: m.parent()?.to_path_buf(), files: BTreeMap::new() })
        })
        .collect();
    // deepest first, so a nested crate's file is its own
    out.sort_by_key(|c| std::cmp::Reverse(c.dir.components().count()));
    for f in rs {
        if let Some(c) = out.iter_mut().find(|c| f.starts_with(&c.dir)) {
            let n = std::fs::read_to_string(&f).map(|s| s.lines().count()).unwrap_or(0);
            let rel = f.strip_prefix(&c.dir).unwrap().to_string_lossy().into_owned();
            c.files.insert(rel, n);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn package_name(toml: &str) -> Option<String> {
    let mut in_pkg = false;
    for l in toml.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            in_pkg = l == "[package]";
        } else if in_pkg && l.starts_with("name") {
            return l.split('"').nth(1).map(str::to_string);
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct Over {
    pub file: PathBuf,
    pub line: usize,
    pub what: String,
}

/// Files and crates over a limit today, and plan rows that would put them over
pub fn check(crates: &[Crate], plans: &[&crate::spec::Scenario]) -> Vec<Over> {
    let mut out = vec![];
    for c in crates {
        for (f, n) in &c.files {
            if *n > FILE_MAX {
                out.push(Over { file: c.dir.join(f), line: 1, what: format!("{f} is {n} lines, over the file limit of {FILE_MAX}") });
            }
        }
        // the crate as its plans leave it: each planned file at its budget, the rest as they are
        let mut planned = c.files.clone();
        let mut by: Vec<String> = vec![];
        for s in plans {
            for r in s.plan.iter().filter(|r| r.krate == c.name) {
                if r.lines > FILE_MAX {
                    out.push(Over { file: s.file.clone(), line: s.line, what: format!("{} plans {} at {} lines, over the file limit of {FILE_MAX}", s.id, r.file, r.lines) });
                }
                let now = planned.get(&r.file).copied().unwrap_or(0);
                planned.insert(r.file.clone(), now.max(r.lines));
                by.push(s.id.clone());
            }
        }
        let total: usize = planned.values().sum();
        let lib = c.dir.join(if c.files.contains_key("src/lib.rs") { "src/lib.rs" } else { "src/main.rs" });
        if c.lines() > CRATE_MAX {
            out.push(Over { file: lib.clone(), line: 1, what: format!("crate {} is {} lines, over the crate limit of {CRATE_MAX}", c.name, c.lines()) });
        } else if total > CRATE_MAX {
            by.dedup();
            out.push(Over { file: lib, line: 1, what: format!("crate {} would be {total} lines with {}'s plan, over the crate limit of {CRATE_MAX}: split it first", c.name, by.join(", ")) });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names() {
        assert_eq!(package_name("[package]\nname = \"a-b\"\n[dependencies]\nname = \"x\"").as_deref(), Some("a-b"));
        assert_eq!(package_name("[workspace]\nmembers = []"), None);
    }
}
