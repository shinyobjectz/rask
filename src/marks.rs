//! Tasks and stubs in Rust, found by tree-sitter. A task is a comment that names its scenario:
//!
//! ```rust,ignore
//! // rask: SH1b the bind group for the user's uniforms
//! fn bind_group(..) -> BindGroup { todo!("rask: SH1b") }
//! ```
//!
//! A stub (`todo!`, `unimplemented!`, a TODO/FIXME/XXX comment) belongs to a task when it says `rask: ID`, or when a
//! task comment sits on its function or just above it. Any other stub is loose: work nobody planned, which fails the
//! check.

use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// a `rask: ID` comment
    Task,
    /// a stub inside a task
    Planned,
    /// a stub outside any task
    Loose,
}

#[derive(Debug, Clone)]
pub struct Mark {
    pub kind: Kind,
    pub id: String,
    pub text: String,
    pub file: PathBuf,
    /// 1-based
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

const STUB_MACROS: &[&str] = &["todo", "unimplemented"];
const STUB_WORDS: &[&str] = &["TODO", "FIXME", "XXX"];

/// Every mark in one Rust source
pub fn scan(file: &Path, src: &str) -> Vec<Mark> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the Rust grammar loads");
    let Some(tree) = parser.parse(src, None) else { return vec![] };
    let mut tasks = vec![];
    let mut stubs = vec![];
    collect(tree.root_node(), src, file, &mut tasks, &mut stubs);
    // a stub with no id of its own takes the task on its function, or the one just above it
    for (m, span) in &mut stubs {
        if m.kind == Kind::Planned {
            continue;
        }
        let near = tasks.iter().filter(|t: &&Mark| t.line + 3 >= span.0 && t.line <= span.1).last();
        if let Some(t) = near {
            m.kind = Kind::Planned;
            m.id = t.id.clone();
        }
    }
    tasks.into_iter().chain(stubs.into_iter().map(|(m, _)| m)).collect()
}

fn collect(node: Node, src: &str, file: &Path, tasks: &mut Vec<Mark>, stubs: &mut Vec<(Mark, (usize, usize))>) {
    let text = |n: Node| &src[n.byte_range()];
    match node.kind() {
        "line_comment" | "block_comment" => {
            let t = text(node);
            if let Some((id, rest)) = rask_id(t) {
                tasks.push(mark(node, file, Kind::Task, id, rest));
            } else if STUB_WORDS.iter().any(|w| has_word(t, w)) {
                let body = t.trim_start_matches(['/', '!', '*']).trim().to_string();
                stubs.push((mark(node, file, Kind::Loose, String::new(), body), span(node)));
            }
        }
        "macro_invocation" => {
            let name = node.child_by_field_name("macro").map(text).unwrap_or("");
            if STUB_MACROS.contains(&name) {
                let t = text(node);
                let (kind, id) = match rask_id(t) {
                    Some((id, _)) => (Kind::Planned, id),
                    None => (Kind::Loose, String::new()),
                };
                stubs.push((mark(node, file, kind, id, format!("{name}!")), span(node)));
            }
        }
        _ => {}
    }
    let mut c = node.walk();
    for child in node.children(&mut c) {
        collect(child, src, file, tasks, stubs);
    }
}

/// The lines of the function around a node (or the node's own lines outside any function)
fn span(node: Node) -> (usize, usize) {
    let mut n = Some(node);
    while let Some(x) = n {
        if x.kind() == "function_item" {
            return (x.start_position().row + 1, x.end_position().row + 1);
        }
        n = x.parent();
    }
    (node.start_position().row + 1, node.end_position().row + 1)
}

fn mark(node: Node, file: &Path, kind: Kind, id: String, text: String) -> Mark {
    let (s, e) = (node.start_position(), node.end_position());
    Mark { kind, id, text, file: file.to_path_buf(), line: s.row + 1, col: s.column + 1, end_line: e.row + 1, end_col: e.column + 1 }
}

/// `rask: SH1b the rest` anywhere in the text: the id and what follows
pub fn rask_id(t: &str) -> Option<(String, String)> {
    let at = t.find("rask:")?;
    let rest = t[at + 5..].trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(rest.len());
    let id = &rest[..end];
    let ok = id.starts_with(|c: char| c.is_ascii_uppercase()) && id.contains(|c: char| c.is_ascii_digit());
    ok.then(|| (id.to_string(), rest[end..].trim_matches(|c: char| c == '"' || c == ')' || c.is_whitespace() || c == '*' || c == '/').to_string()))
}

fn has_word(t: &str, w: &str) -> bool {
    t.match_indices(w).any(|(i, _)| {
        let before = t[..i].chars().next_back().is_none_or(|c| !c.is_ascii_alphanumeric());
        let after = t[i + w.len()..].chars().next().is_none_or(|c| !c.is_ascii_alphanumeric());
        before && after
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
// rask: SH1b the bind group
fn bind() -> u32 {
    todo!()
}

fn loose() {
    unimplemented!()
}

fn named() { todo!("rask: SH2 later") }

// TODO: someday
fn fine() { let todo = 1; let _ = todo; }
"#;

    #[test]
    fn tasks_planned_and_loose() {
        let m = scan(Path::new("a.rs"), SRC);
        let got: Vec<_> = m.iter().map(|m| (m.kind.clone(), m.id.as_str(), m.line)).collect();
        assert!(got.contains(&(Kind::Task, "SH1b", 2)));
        assert!(got.contains(&(Kind::Planned, "SH1b", 4)));
        assert!(got.contains(&(Kind::Loose, "", 8)));
        assert!(got.contains(&(Kind::Planned, "SH2", 11)));
        assert!(got.contains(&(Kind::Loose, "", 13)));
        assert_eq!(m.len(), 5);
    }
}
