//! The commands: each reads the scenarios (spec), the marks in the code (marks), the sizes (budget) and the board.

use crate::board::{self, Board, Entry};
use crate::budget;
use crate::diag::{Finding, Level};
use crate::marks::{self, Kind, Mark};
use crate::spec::{self, Scenario};
use std::path::Path;

struct World {
    scenarios: Vec<Scenario>,
    marks: Vec<Mark>,
    board: Board,
}

fn world(root: &Path) -> Result<World, String> {
    let scenarios = spec::all(root)?;
    let mut all = vec![];
    crate::walk::files(root, &mut |p| {
        if p.extension().is_some_and(|x| x == "rs") {
            if let Ok(src) = std::fs::read_to_string(p) {
                all.extend(marks::scan(p, &src));
            }
        }
    });
    Ok(World { scenarios, marks: all, board: Board::open(root)? })
}

impl World {
    /// `ID` or `FILE:ID`; an id two feature files share must be given with its file
    fn find(&self, root: &Path, key: &str) -> Result<&Scenario, String> {
        let (file, id) = key.rsplit_once(':').map_or((None, key), |(f, i)| (Some(f), i));
        let hits: Vec<_> = self.scenarios.iter().filter(|s| s.id == id && file.is_none_or(|f| s.file.ends_with(f))).collect();
        match hits.as_slice() {
            [one] => Ok(one),
            [] => Err(format!("no scenario {key} in any feature file under {}", root.display())),
            many => Err(format!("{id} is in {}: give it as FILE:{id}", many.iter().map(|s| rel(root, &s.file)).collect::<Vec<_>>().join(", "))),
        }
    }
    fn open_marks(&self, id: &str) -> Vec<&Mark> {
        self.marks.iter().filter(|m| m.id == id && m.kind != Kind::Loose).collect()
    }
    fn waiting(&self, s: &Scenario) -> Vec<String> {
        s.needs().into_iter().filter(|n| self.board.get(n).state != "done").collect()
    }
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

pub fn ls(root: &Path, all: bool, lane: Option<&str>, ready: bool) -> Result<i32, String> {
    let w = world(root)?;
    println!("{:<8} {:<8} {:<12} {:>5}  {:<24} {}", "ID", "STATE", "LANE", "MARKS", "FEATURE", "TITLE");
    for s in &w.scenarios {
        let e = w.board.get(&s.id);
        let marks = w.open_marks(&s.id).len();
        let in_play = e.state != "open" && e.state != "done" || marks > 0 || !s.plan.is_empty() && e.state != "done";
        if ready && !(e.state == "open" && w.waiting(s).is_empty() && (marks > 0 || !s.plan.is_empty())) {
            continue;
        }
        if !ready && !all && !in_play {
            continue;
        }
        if lane.is_some_and(|l| l != e.lane) {
            continue;
        }
        let feature = rel(root, &s.file).trim_start_matches("docs/").to_string();
        println!("{:<8} {:<8} {:<12} {:>5}  {:<24} {}", s.id, e.state, if e.lane.is_empty() { "-" } else { &e.lane }, marks, feature, s.title);
    }
    Ok(0)
}

pub fn show(root: &Path, key: &str) -> Result<i32, String> {
    let w = world(root)?;
    let s = w.find(root, key)?;
    let e = w.board.get(&s.id);
    println!("{}. {}\n  {}:{}\n  state {}{}", s.id, s.title, rel(root, &s.file), s.line, e.state, if e.lane.is_empty() { String::new() } else { format!(", lane {}", e.lane) });
    if !s.needs().is_empty() {
        println!("  needs {} (open: {})", s.needs().join(", "), w.waiting(s).join(", "));
    }
    for r in &s.plan {
        println!("  plan  {} {} ({} lines): {}", r.krate, r.file, r.lines, r.job);
    }
    for m in w.open_marks(&s.id) {
        println!("  mark  {}:{} {}", rel(root, &m.file), m.line, m.text);
    }
    print!("{}", w.board.log(&s.id));
    Ok(0)
}

pub fn check(root: &Path, json: bool, cargo: Option<Vec<String>>) -> Result<i32, String> {
    if let Some(extra) = cargo {
        // cargo's own messages first, passed through as they come, so rust-analyzer sees both
        let status = std::process::Command::new("cargo")
            .args(["check", "--message-format=json-diagnostic-rendered-ansi"])
            .args(&extra)
            .current_dir(root)
            .status()
            .map_err(|e| format!("cargo: {e}"))?;
        let _ = status;
    }
    let w = world(root)?;
    let mut out: Vec<Finding> = vec![];
    let ids: std::collections::BTreeSet<&str> = w.scenarios.iter().map(|s| s.id.as_str()).collect();
    for m in &w.marks {
        let mut f = Finding::at(Level::Note, "task", String::new(), &m.file, root, m.line);
        (f.col, f.end_line, f.end_col) = (m.col, m.end_line, m.end_col);
        match m.kind {
            Kind::Loose => {
                f.level = Level::Error;
                f.code = "loose-stub";
                f.message = format!("{}: a stub no task owns; finish it, or mark the task it belongs to with `// rask: ID what`", m.text);
            }
            _ if !ids.contains(m.id.as_str()) => {
                f.level = Level::Error;
                f.code = "no-scenario";
                f.message = format!("rask: {} names no scenario; add `Scenario: {}.` to a feature file", m.id, m.id);
            }
            Kind::Task => {
                let e = w.board.get(&m.id);
                f.message = format!("{} open ({}{}): {}", m.id, e.state, if e.lane.is_empty() { String::new() } else { format!(", {}", e.lane) }, m.text);
            }
            Kind::Planned => {
                f.code = "planned-stub";
                f.message = format!("{} stays until {} is done", m.text, m.id);
            }
        }
        out.push(f);
    }
    let crates = budget::crates(root);
    let planned: Vec<&Scenario> = w.scenarios.iter().filter(|s| !s.plan.is_empty() && w.board.get(&s.id).state != "done").collect();
    for o in budget::check(&crates, &planned) {
        out.push(Finding::at(Level::Error, "size", o.what, &o.file, root, o.line));
    }
    out.sort_by(|a, b| (a.file.as_str(), a.line).cmp(&(b.file.as_str(), b.line)));
    let errors = out.iter().filter(|f| f.level == Level::Error).count();
    for f in &out {
        println!("{}", if json { f.json() } else { f.line() });
    }
    if !json {
        let tasks = out.iter().filter(|f| f.code == "task").count();
        eprintln!("rask: {errors} error(s), {tasks} open task mark(s)");
    }
    Ok(if errors > 0 { 1 } else { 0 })
}

pub fn plan(root: &Path, key: &str) -> Result<i32, String> {
    let w = world(root)?;
    let s = w.find(root, key)?;
    for line in crate::plan::run(root, s, &budget::crates(root))? {
        println!("{line}");
    }
    Ok(0)
}

pub fn claim(root: &Path, key: &str, lane: &str) -> Result<i32, String> {
    let w = world(root)?;
    let s = w.find(root, key)?;
    let mut e = w.board.get(&s.id);
    if e.state == "done" {
        return Err(format!("{} is done", s.id));
    }
    if !e.lane.is_empty() && e.lane != lane {
        return Err(format!("{} is {}'s (it gives it back with: rask open {})", s.id, e.lane, s.id));
    }
    let wait = w.waiting(s);
    if !wait.is_empty() {
        return Err(format!("{} needs {} done first", s.id, wait.join(", ")));
    }
    board::branch(root, lane)?;
    (e.state, e.lane) = ("claimed".into(), lane.into());
    w.board.put(&s.id, &e, &format!("claimed by {lane}"))?;
    println!("{} is {lane}'s: commit to branch {lane}, each message starting \"{}: \"", s.id, s.id);
    Ok(0)
}

pub fn set(root: &Path, key: &str, state: &str, why: &str) -> Result<i32, String> {
    let w = world(root)?;
    let s = w.find(root, key)?;
    let mut e = w.board.get(&s.id);
    e.state = state.into();
    if state == "open" {
        e.lane.clear();
    }
    w.board.put(&s.id, &e, why)?;
    Ok(0)
}

pub fn note(root: &Path, key: &str, text: &str) -> Result<i32, String> {
    let w = world(root)?;
    let s = w.find(root, key)?;
    let e = w.board.get(&s.id);
    w.board.put(&s.id, &e, text)?;
    Ok(0)
}

pub fn done(root: &Path, key: &str, main: &str) -> Result<i32, String> {
    let w = world(root)?;
    let s = w.find(root, key)?;
    let wait = w.waiting(s);
    if !wait.is_empty() {
        return Err(format!("{} needs {} done first", s.id, wait.join(", ")));
    }
    if let Some((remote, _)) = main.split_once('/') {
        let _ = std::process::Command::new("git").args(["fetch", "-q", remote]).current_dir(root).status();
    }
    let why = board::not_done(root, main, &s.id, &rel(root, &s.file))?;
    if !why.is_empty() {
        return Err(format!("{} isn't done:\n  {}", s.id, why.join("\n  ")));
    }
    let shas = board::commits(root, main, &s.id)?;
    let e = Entry { state: "done".into(), lane: w.board.get(&s.id).lane, commits: shas.iter().map(|c| &c[..9]).collect::<Vec<_>>().join(" ") };
    w.board.put(&s.id, &e, &format!("done: {} commit(s) on {main}", shas.len()))?;
    println!("{} done", s.id);
    Ok(0)
}
