//! The files rask reads: what git tracks or would track under the root (ignored files left out), or in a folder that
//! isn't a repository, everything but build output, dependencies and hidden folders. Vendored code is never ours.

use std::path::{Path, PathBuf};

const SKIP: &[&str] = &["target", "node_modules", "vendor", ".git"];

pub fn files(root: &Path, each: &mut dyn FnMut(&Path)) {
    let git = std::process::Command::new("git")
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", ".", ":!vendor/*", ":!*/vendor/*"])
        .current_dir(root)
        .output();
    match git {
        Ok(out) if out.status.success() => {
            for name in out.stdout.split(|b| *b == 0).filter(|n| !n.is_empty()) {
                let p: PathBuf = root.join(String::from_utf8_lossy(name).as_ref());
                if p.is_file() && !p.is_symlink() {
                    each(&p);
                }
            }
        }
        _ => walk(root, each),
    }
}

fn walk(dir: &Path, each: &mut dyn FnMut(&Path)) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for e in read.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() && !name.starts_with('.') && !SKIP.contains(&name.as_ref()) {
            walk(&p, each);
        } else if kind.is_file() {
            each(&p);
        }
    }
}
