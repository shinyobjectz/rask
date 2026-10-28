//! `rask plan FILE:ID`: a scenario's plan laid out as Rust files. Each row's file is made when it's missing, with its
//! job as the module doc and one task comment for the scenario; a file that exists gets the task comment at its end.
//! Nothing is added to a lib.rs or main.rs: a `mod` line goes in once the file compiles, so a plan never breaks a
//! build. Refused, with nothing written, when the plan puts a file or crate over its limit.

use crate::budget::{self, Crate};
use crate::spec::Scenario;
use std::path::Path;

pub fn run(root: &Path, s: &Scenario, crates: &[Crate]) -> Result<Vec<String>, String> {
    if s.plan.is_empty() {
        return Err(format!("{} has no plan: add a \"Given the plan\" table (crate | file | lines | job) under it", s.id));
    }
    let over = budget::check(crates, &[s]);
    let ours: Vec<_> = over.iter().filter(|o| o.what.contains(&s.id)).collect();
    if !ours.is_empty() {
        return Err(ours.iter().map(|o| o.what.clone()).collect::<Vec<_>>().join("\n"));
    }
    let mut out = vec![];
    for r in &s.plan {
        let c = crates.iter().find(|c| c.name == r.krate).ok_or(format!("{}: no crate {} under {}", s.id, r.krate, root.display()))?;
        let path = c.dir.join(&r.file);
        let task = format!("// rask: {} {}\n", s.id, r.job);
        let rel = path.strip_prefix(root).unwrap_or(&path).display().to_string();
        if path.exists() {
            let now = std::fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?;
            if now.contains(&format!("rask: {} ", s.id)) {
                out.push(format!("{rel}: already marked"));
                continue;
            }
            std::fs::write(&path, format!("{}\n{task}", now.trim_end())).map_err(|e| format!("{rel}: {e}"))?;
            out.push(format!("{rel}: task added ({} lines now, {} planned)", now.lines().count(), r.lines));
        } else {
            if let Some(d) = path.parent() {
                std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            std::fs::write(&path, format!("//! {}\n\n{task}", r.job)).map_err(|e| format!("{rel}: {e}"))?;
            let module = Path::new(&r.file).file_stem().map(|m| m.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(format!("{rel}: made ({} lines planned); add `mod {module};` once it compiles", r.lines));
        }
    }
    Ok(out)
}
