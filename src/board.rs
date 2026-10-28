//! Who has which task, kept outside git (`~/.local/state/rask/<repo>/<ID>`) so every branch and session sees a claim
//! the moment it's made. A task is a scenario. A lane claims it, and its GitButler branch (named for the lane) carries
//! the commits, each starting "<ID>: ". Done is checked, never declared: see `done`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Board {
    dir: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub state: String,
    pub lane: String,
    pub commits: String,
}

impl Board {
    pub fn open(root: &Path) -> Result<Board, String> {
        let base = std::env::var_os("RASK_BOARD").map(PathBuf::from).unwrap_or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
            home.join(".local/state/rask")
        });
        let key = root.canonicalize().unwrap_or(root.to_path_buf()).to_string_lossy().trim_start_matches('/').replace('/', "-");
        let dir = base.join(key);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Board { dir })
    }

    pub fn get(&self, id: &str) -> Entry {
        let kv: BTreeMap<String, String> = std::fs::read_to_string(self.dir.join(id))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect();
        let v = |k: &str| kv.get(k).cloned().unwrap_or_default();
        Entry { state: if v("state").is_empty() { "open".into() } else { v("state") }, lane: v("lane"), commits: v("commits") }
    }

    pub fn put(&self, id: &str, e: &Entry, why: &str) -> Result<(), String> {
        let tmp = self.dir.join(format!(".{id}.new"));
        std::fs::write(&tmp, format!("state={}\nlane={}\ncommits={}\n", e.state, e.lane, e.commits)).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, self.dir.join(id)).map_err(|e| e.to_string())?;
        let stamp = Command::new("date").arg("+%m-%d %H:%M").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let mut log = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join(format!("{id}.log"))).map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut log, format!("{stamp} {why}\n").as_bytes()).map_err(|e| e.to_string())
    }

    pub fn log(&self, id: &str) -> String {
        std::fs::read_to_string(self.dir.join(format!("{id}.log"))).unwrap_or_default()
    }
}

/// The lane's GitButler branch, made when it isn't there. `but branch list` draws stacks (`*-shaders`, `└─sdk`): a
/// name is a word with its leading marks taken off
pub fn branch(root: &Path, lane: &str) -> Result<(), String> {
    let list = Command::new("but").args(["branch", "list"]).current_dir(root).output().map_err(|e| format!("but: {e}"))?;
    let have = String::from_utf8_lossy(&list.stdout).lines().any(|l| l.split_whitespace().any(|w| w.trim_start_matches(|c: char| !c.is_alphanumeric()) == lane));
    if have {
        return Ok(());
    }
    let made = Command::new("but").args(["branch", "new", lane]).current_dir(root).status().map_err(|e| format!("but: {e}"))?;
    made.success().then_some(()).ok_or(format!("but branch new {lane} failed"))
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git").args(args).current_dir(root).output().map_err(|e| format!("git: {e}"))?;
    if !o.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The commits on `main` whose message starts with "<ID>:" (or names it as "[ID]"), oldest first
pub fn commits(root: &Path, main: &str, id: &str) -> Result<Vec<String>, String> {
    let pat = format!("^{id}:|\\[{id}\\]");
    Ok(git(root, &["log", "--reverse", "--format=%H", "-E", &format!("--grep={pat}"), main])?.lines().map(str::to_string).collect())
}

/// Why `id` isn't done on `main`, or nothing when it is: its commits are there, they changed its feature file (the
/// scenario's status, written where the work is), no task mark for it is left in main's Rust, and nothing those
/// files they changed holds a loose stub
pub fn not_done(root: &Path, main: &str, id: &str, feature: &str) -> Result<Vec<String>, String> {
    let mut why = vec![];
    let shas = commits(root, main, id)?;
    if shas.is_empty() {
        why.push(format!("no commit on {main} starts \"{id}: \" (land the branch first)"));
        return Ok(why);
    }
    let touched = shas.iter().any(|s| git(root, &["diff-tree", "--no-commit-id", "--name-only", "-r", s]).is_ok_and(|o| o.lines().any(|l| l == feature)));
    if !touched {
        why.push(format!("its commits don't change {feature}: write the scenario's status there (what was built, what proves it)"));
    }
    let left = git(root, &["grep", "-n", "-E", &format!("rask: {id}([^A-Za-z0-9-]|$)"), main, "--", "*.rs"]).unwrap_or_default();
    for l in left.lines().take(5) {
        why.push(format!("still marked on {main}: {}", l.trim_start_matches(&format!("{main}:"))));
    }
    // the files its commits changed, as main has them now: a stub one commit added and a later one finished is fine
    let mut files: Vec<String> = vec![];
    for s in &shas {
        for f in git(root, &["diff-tree", "--no-commit-id", "--name-only", "-r", s, "--", "*.rs"])?.lines() {
            if !files.iter().any(|x| x == f) {
                files.push(f.to_string());
            }
        }
    }
    for f in &files {
        let Ok(src) = git(root, &["show", &format!("{main}:{f}")]) else { continue };
        for m in crate::marks::scan(Path::new(f), &src).into_iter().filter(|m| m.kind == crate::marks::Kind::Loose) {
            why.push(format!("loose stub on {main} at {f}:{}: {}", m.line, m.text));
        }
    }
    Ok(why)
}
