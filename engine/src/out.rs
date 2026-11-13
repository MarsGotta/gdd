//! What a command says: short plain lines for the agent, and the same facts for a program.

use serde_json::{Map, Value, json};

/// The result of one command: the lines it prints, the facts of its last line and its exit code.
///
/// Exit codes: 0 fine, 1 refused or a check failed or a cap spent, 2 called wrong or a file missing.
#[derive(Debug, Clone, Default)]
pub struct Out {
    pub lines: Vec<String>,
    pub facts: Map<String, Value>,
    pub code: i32,
    /// When the command refused, a short code for why, such as `needs-approval`.
    pub refused: Option<String>,
    /// What the journal records beyond the invocation, such as what a launch saw.
    pub journal: Option<Value>,
    /// Bookkeeping to commit on the main branch once the call is journaled: the message.
    pub commit_log: Option<String>,
}

impl Out {
    pub fn new(command: &str) -> Out {
        let mut facts = Map::new();
        facts.insert("command".into(), json!(command));
        Out {
            facts,
            ..Out::default()
        }
    }

    pub fn say(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    pub fn fact(&mut self, key: &str, value: impl Into<Value>) {
        self.facts.insert(key.into(), value.into());
    }

    /// The last line, `gdd {json}`.
    pub fn last_line(&self) -> String {
        let mut facts = self.facts.clone();
        facts.insert("exit".into(), json!(self.code));
        if let Some(r) = &self.refused {
            facts.insert("refused".into(), json!(r));
        }
        format!("gdd {}", Value::Object(facts))
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        for l in &self.lines {
            s.push_str(l);
            s.push('\n');
        }
        s.push_str(&self.last_line());
        s.push('\n');
        s
    }

    /// Turn a failure into what the command prints, keeping the facts gathered so far.
    pub fn with_fail(mut self, fail: Fail) -> Out {
        self.lines.extend(fail.lines);
        self.code = fail.exit;
        if fail.exit == 1 && !fail.reason.is_empty() {
            self.refused = Some(fail.reason.clone());
        }
        self.fact("reason", fail.reason);
        if let Some(w) = fail.what {
            self.fact("what", w);
        }
        self
    }
}

/// A command that cannot go on: called wrong, a file missing, or refused by the state.
#[derive(Debug, Clone)]
pub struct Fail {
    pub exit: i32,
    /// A short code: `needs-approval`, `cap-spent`, `not-merged`, `out-of-order`, `usage`, `missing`...
    pub reason: String,
    pub lines: Vec<String>,
    /// What is missing, by name: a file, a ref, a commit, a step.
    pub what: Option<String>,
}

impl Fail {
    /// Called wrong: exit 2.
    pub fn usage(why: impl Into<String>) -> Fail {
        Fail {
            exit: 2,
            reason: "usage".into(),
            lines: vec![why.into()],
            what: None,
        }
    }

    /// Something that isn't there, named in `what` (a file, a ref, a commit, a step), and why it
    /// matters: exit 2. The printed line always names it.
    pub fn missing(what: impl Into<String>, why: impl Into<String>) -> Fail {
        let what = what.into();
        let why = why.into();
        let line = if why.contains(&what) {
            why
        } else if why.is_empty() {
            format!("{what} is missing")
        } else {
            format!("{what}: {why}")
        };
        Fail {
            exit: 2,
            reason: "missing".into(),
            lines: vec![line],
            what: Some(what),
        }
    }

    /// Refused by the state, with why and what to do instead: exit 1.
    pub fn refuse(reason: &str, why: impl Into<String>, instead: impl Into<String>) -> Fail {
        let instead = instead.into();
        let mut lines = vec![format!("refused: {}", why.into())];
        if !instead.is_empty() {
            lines.push(format!("instead: {instead}"));
        }
        Fail {
            exit: 1,
            reason: reason.into(),
            lines,
            what: None,
        }
    }
}

/// A file this command couldn't read or write, named.
pub fn io(path: &std::path::Path, e: std::io::Error) -> Fail {
    Fail::missing(
        path.display().to_string(),
        format!("{}: {e}", path.display()),
    )
}

pub type R<T> = Result<T, Fail>;
