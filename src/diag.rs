//! Findings as rustc's JSON diagnostics, one per line, which rust-analyzer's flycheck reads beside cargo's own (set
//! `rust-analyzer.check.overrideCommand` to `["rask", "check", "--cargo", "--json"]`), or as plain lines for a terminal.

use serde_json::json;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// shown as information: an open task
    Note,
    /// fails the check: a loose stub, a file or crate over its limit, a task with no scenario
    Error,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub level: Level,
    pub code: &'static str,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl Finding {
    pub fn at(level: Level, code: &'static str, message: String, file: &Path, root: &Path, line: usize) -> Self {
        let file = file.strip_prefix(root).unwrap_or(file).to_string_lossy().into_owned();
        Finding { level, code, message, file, line, col: 1, end_line: line, end_col: 1 }
    }

    pub fn json(&self) -> String {
        let level = match self.level {
            Level::Note => "note",
            Level::Error => "error",
        };
        json!({
            "$message_type": "diagnostic",
            "message": self.message,
            "code": { "code": format!("rask::{}", self.code), "explanation": null },
            "level": level,
            "spans": [{
                "file_name": self.file,
                "byte_start": 0, "byte_end": 0,
                "line_start": self.line, "line_end": self.end_line,
                "column_start": self.col, "column_end": self.end_col.max(self.col),
                "is_primary": true, "text": [], "label": null,
                "suggested_replacement": null, "suggestion_applicability": null, "expansion": null
            }],
            "children": [],
            "rendered": format!("{level}[rask::{}]: {}\n --> {}:{}:{}\n", self.code, self.message, self.file, self.line, self.col),
        })
        .to_string()
    }

    pub fn line(&self) -> String {
        let tag = match self.level {
            Level::Note => "note",
            Level::Error => "error",
        };
        format!("{}:{}: {tag}[{}] {}", self.file, self.line, self.code, self.message)
    }
}
