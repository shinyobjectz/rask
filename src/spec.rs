//! The plan, read from feature files. A scenario is a task: `Scenario: SH1. A lit user shader` names it SH1. Its plan
//! is a table under a step that says "the plan", one row per Rust file it adds or grows:
//!
//! ```gherkin
//! Scenario: SH1. A lit user shader on StandardMaterial
//!   Given the plan
//!     | crate             | file                | lines | job                                         |
//!     | raea-sys-material | src/shader_ext.rs   | 260   | the user's extension over StandardMaterial  |
//! ```
//!
//! `lines` is the file's budget once the scenario is done; the planner refuses a plan that puts a file over the file
//! limit or a crate over the crate limit.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Scenario {
    pub id: String,
    pub title: String,
    pub file: PathBuf,
    pub line: usize,
    /// its Gherkin tags, `@needs:SH1,SH2` naming the scenarios that land first
    pub tags: Vec<String>,
    pub plan: Vec<PlanRow>,
}

impl Scenario {
    pub fn needs(&self) -> Vec<String> {
        self.tags.iter().filter_map(|t| t.strip_prefix("@needs:")).flat_map(|n| n.split(',')).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect()
    }
}

#[derive(Debug, Clone)]
pub struct PlanRow {
    pub krate: String,
    pub file: String,
    pub lines: usize,
    pub job: String,
}

/// Every scenario with an id in one feature file
pub fn read(path: &Path) -> Result<Vec<Scenario>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(path, &text)
}

/// Every scenario with an id in a feature file's text (`path` is where it's said to be)
pub fn parse(path: &Path, text: &str) -> Result<Vec<Scenario>, String> {
    let mut out: Vec<Scenario> = vec![];
    let mut in_plan = false;
    let mut header: Option<Vec<String>> = None;
    let mut tags: Vec<String> = vec![];
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some((id, title)) = scenario_line(line) {
            out.push(Scenario { id, title, file: path.to_path_buf(), line: i + 1, tags: std::mem::take(&mut tags), plan: vec![] });
            in_plan = false;
            header = None;
            continue;
        }
        if line.starts_with('@') {
            tags.extend(line.split_whitespace().filter(|t| t.starts_with('@')).map(str::to_string));
            continue;
        }
        if line.starts_with('|') {
            if !in_plan {
                continue;
            }
            let cells: Vec<String> = line.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
            let Some(h) = &header else {
                header = Some(cells.iter().map(|c| c.to_lowercase()).collect());
                continue;
            };
            let col = |name: &str| h.iter().position(|c| c == name).and_then(|i| cells.get(i)).cloned().unwrap_or_default();
            let lines = col("lines").parse().map_err(|_| format!("{}:{}: plan row without a number in `lines`", path.display(), i + 1))?;
            let row = PlanRow { krate: col("crate"), file: col("file"), lines, job: col("job") };
            if row.krate.is_empty() || row.file.is_empty() || row.job.is_empty() {
                return Err(format!("{}:{}: a plan row needs crate, file, lines and job", path.display(), i + 1));
            }
            if let Some(s) = out.last_mut() {
                s.plan.push(row);
            }
            continue;
        }
        // tags belong to the scenario right below them
        if !line.is_empty() && !line.starts_with('#') {
            tags.clear();
        }
        // a step that says "the plan" opens a table; any other step or keyword closes it
        in_plan = !line.is_empty() && is_step(line) && line.to_lowercase().contains("the plan");
        header = None;
    }
    Ok(out)
}

/// Every scenario under `root`'s feature files (docs/ and any folder, vendored and generated ones left out)
pub fn all(root: &Path) -> Result<Vec<Scenario>, String> {
    let mut files = vec![];
    crate::walk::files(root, &mut |p| {
        if p.extension().is_some_and(|x| x == "feature") {
            files.push(p.to_path_buf());
        }
    });
    files.sort();
    let mut out = vec![];
    for f in files {
        out.extend(read(&f)?);
    }
    Ok(out)
}

/// `Scenario: SH1. Title`, `Scenario Outline: B5a: Title`: the id is letters then digits, then an optional letter
fn scenario_line(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("Scenario Outline:").or_else(|| line.strip_prefix("Scenario:"))?.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(rest.len());
    let id = &rest[..end];
    let letters = id.chars().take_while(|c| c.is_ascii_uppercase()).count();
    if letters == 0 || !id[letters..].starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let title = rest[end..].trim_start_matches(['.', ':']).trim().to_string();
    Some((id.to_string(), title))
}

fn is_step(line: &str) -> bool {
    ["Given ", "When ", "Then ", "And ", "But ", "* "].iter().any(|k| line.starts_with(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_plans() {
        let dir = std::env::temp_dir().join(format!("rask-spec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.feature");
        std::fs::write(
            &f,
            "Feature: x\n  @needs:B5a,X1 @wip\n  Scenario: SH1. A lit shader\n    Given the plan\n      | crate | file | lines | job |\n      | m | src/a.rs | 200 | the extension |\n    Then it draws\n      | not | a | plan |\n  Scenario: B5a: Other\n  Scenario: no id here\n",
        )
        .unwrap();
        let s = read(&f).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].id.as_str(), s[0].title.as_str(), s[0].plan.len()), ("SH1", "A lit shader", 1));
        assert_eq!((s[0].plan[0].lines, s[0].plan[0].file.as_str()), (200, "src/a.rs"));
        assert_eq!((s[1].id.as_str(), s[1].title.as_str()), ("B5a", "Other"));
        assert_eq!(s[0].needs(), vec!["B5a", "X1"]);
        assert!(s[1].tags.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
