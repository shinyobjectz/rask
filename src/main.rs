//! rask: tasks planned in feature files, marked in Rust, checked for size and stubs, claimed by lanes on GitButler
//! branches, and shown in rust-analyzer.
//!
//!   rask ls [--all] [--lane L]     tasks in play: claimed, blocked, or open with a plan or marks (--all: every scenario)
//!   rask ready                     open tasks whose @needs are done: what a lane can take next
//!   rask show ID                   the scenario, its plan, its marks in the code, its log
//!   rask check [--json] [--cargo [ARGS..]]
//!                                  loose stubs, files and crates over their limits, marks with no scenario, open tasks;
//!                                  --cargo runs `cargo check --message-format=json ARGS` first (rust-analyzer's
//!                                  overrideCommand: ["rask", "check", "--cargo", "--json"]); exits 1 on an error
//!   rask plan [FILE:]ID            the scenario's plan made into files with task marks (refused over a limit)
//!   rask claim ID LANE             the lane takes it; its GitButler branch is made if missing (refused on open @needs)
//!   rask block ID WHY | rask open ID | rask note ID TEXT
//!   rask done ID [--main REF]      closed when checked: commits "ID: ..." on REF (origin/main) that change its feature
//!                                  file, no marks left there, no loose stub added
//!
//! The root is the git repository around the current folder (or --root DIR).

mod board;
mod budget;
mod check;
mod diag;
mod marks;
mod plan;
mod spec;
mod walk;

use std::path::PathBuf;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let root = match args.iter().position(|a| a == "--root") {
        Some(i) => {
            let r = args.get(i + 1).cloned().unwrap_or_default();
            args.drain(i..=(i + 1).min(args.len() - 1));
            PathBuf::from(r)
        }
        None => toplevel(),
    };
    match run(&root, &args) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("rask: {e}");
            std::process::exit(2);
        }
    }
}

fn toplevel() -> PathBuf {
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn usage() -> String {
    include_str!("main.rs").lines().skip(3).take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n")
}

fn run(root: &std::path::Path, args: &[String]) -> Result<i32, String> {
    let a = |i: usize| args.get(i).map(String::as_str);
    let flag = |f: &str| args.iter().any(|x| x == f);
    let opt = |f: &str| args.iter().position(|x| x == f).and_then(|i| args.get(i + 1)).cloned();
    let need = |i: usize, what: &str| a(i).map(str::to_string).ok_or(format!("{what} missing\n{}", usage()));
    match a(0) {
        Some("ls") => check::ls(root, flag("--all"), opt("--lane").as_deref(), false),
        Some("ready") => check::ls(root, false, None, true),
        Some("show") => check::show(root, &need(1, "ID")?),
        Some("check") => {
            let cargo = args.iter().position(|x| x == "--cargo").map(|i| args[i + 1..].iter().filter(|x| *x != "--json").cloned().collect());
            check::check(root, flag("--json"), cargo)
        }
        Some("plan") => check::plan(root, &need(1, "ID")?),
        Some("claim") => check::claim(root, &need(1, "ID")?, &need(2, "LANE")?),
        Some("block") => check::set(root, &need(1, "ID")?, "blocked", &format!("blocked: {}", need(2, "WHY")?)),
        Some("open") => check::set(root, &need(1, "ID")?, "open", "open"),
        Some("note") => check::note(root, &need(1, "ID")?, &need(2, "TEXT")?),
        Some("done") => check::done(root, &need(1, "ID")?, opt("--main").as_deref().unwrap_or("origin/main")),
        _ => Err(usage()),
    }
}
