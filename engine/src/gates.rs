//! The gates: which checks a change's color asks for, in which order, and running each one with
//! its command, its exit code, its seconds by the clock and its last lines. Nothing paraphrased.

use crate::blueprint::Blueprint;
use std::path::Path;
use std::process::{Command, Stdio};

pub const SEAL_COMMAND: &str = "git diff --exit-code <start> HEAD -- gdd features AGENTS.md CLAUDE.md .agents .claude <sealed paths> ':(exclude)gdd/log' ':(exclude)gdd/backlog.md'";

#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    pub check: String,
    pub command: String,
    /// `each round` or `before review`.
    pub tier: String,
    /// Why it doesn't run, when it doesn't: its `when` wasn't touched, or it isn't a machine check.
    pub skipped: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ran {
    pub check: String,
    pub command: String,
    pub tier: String,
    pub exit: Option<i32>,
    pub seconds: Option<f64>,
    pub tail: String,
    pub passed: bool,
}

impl Ran {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "check": self.check,
            "seconds": self.seconds,
            "command": self.command,
            "exit": self.exit,
            "tail": self.tail,
            "result": if self.passed { "pass" } else { "fail" },
        })
    }
}

/// Quote a word for `sh`.
pub fn quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:@+=,".contains(c))
    {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// What the layer fills in a check's command, as the contract names it: `<start>` and `<base>`,
/// full commit hashes git resolved, and `<sealed paths>`, each quoted for `sh`, which runs it. A
/// hash is all hex digits, so it comes out as it is, and stays right inside the command's own
/// double quotes too, as in `"<start>^{commit}"` or `--log-opts="<start>..HEAD"`.
#[derive(Debug, Clone, Copy)]
pub struct Fill<'a> {
    pub start: &'a str,
    pub base: Option<&'a str>,
    pub sealed: &'a [String],
}

/// Fill a check's command, or say which placeholder it still holds, which the layer doesn't
/// fill: run as written, it would fail on every change, or judge nothing.
pub fn fill(command: &str, f: &Fill) -> Result<String, String> {
    let paths: Vec<String> = f.sealed.iter().map(|p| quote(p)).collect();
    let paths = paths.join(" ");
    let mut c = if paths.is_empty() {
        command
            .replace(" <sealed paths>", "")
            .replace("<sealed paths>", "")
    } else {
        command.replace("<sealed paths>", &paths)
    };
    if crate::log::is_hash(f.start) {
        c = c.replace("<start>", &quote(f.start));
    }
    if let Some(b) = f.base.filter(|b| crate::log::is_hash(b)) {
        c = c.replace("<base>", &quote(b));
    }
    match placeholder(&c) {
        Some(p) => Err(p),
        None => Ok(c),
    }
}

/// The first `<...>` in a command that reads as a placeholder: a letter after `<`, then words,
/// spaces and the punctuation of a sentence, and no `.` or `/`, which a shell redirection to a
/// file would have.
pub fn placeholder(command: &str) -> Option<String> {
    let b = command.as_bytes();
    for (i, _) in command.match_indices('<') {
        // `<<EOF` is a heredoc, `<(` a process substitution, `< file` a redirection.
        if i > 0 && b[i - 1] == b'<' || !b.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
            continue;
        }
        let rest = &command[i + 1..];
        let Some(end) = rest.find(['>', '<']) else {
            continue;
        };
        if rest.as_bytes()[end] != b'>' {
            continue;
        }
        let inside = &rest[..end];
        if inside.trim_end() == inside
            && inside
                .chars()
                .all(|c| c.is_alphanumeric() || " ,;:'`()_-".contains(c))
        {
            return Some(format!("<{inside}>"));
        }
    }
    None
}

/// The `seal` check's command for a change, with its `<start>` and its sealed paths from the log,
/// each quoted for `sh`, which runs it.
pub fn seal_command(bp: &Blueprint, start: &str, sealed: &[String]) -> String {
    let template = bp
        .check("seal")
        .and_then(|c| c.command)
        .unwrap_or_else(|| SEAL_COMMAND.to_string());
    let paths: Vec<String> = sealed.iter().map(|p| quote(p)).collect();
    template
        .replace("<start>", &quote(start))
        .replace("<sealed paths>", &paths.join(" "))
        .replace("  ", " ")
}

/// The checks a change's gates run, in order: `seal` first, then the rest of what runs each round,
/// then what runs before the review. `touched` says whether the change touched a check's `when`.
/// `zone_checks` are the checks of the standing gates of the zones it touches.
pub fn plan(
    bp: &Blueprint,
    color: &str,
    touched: &dyn Fn(&[String]) -> bool,
    zone_checks: &[String],
) -> Vec<Planned> {
    let mut wanted: Vec<String> = bp.color_checks(color);
    for c in zone_checks {
        if !wanted.contains(c) {
            wanted.push(c.clone());
        }
    }
    let mut out = vec![];
    for id in &wanted {
        let Some(check) = bp.check(id) else {
            out.push(Planned {
                check: id.clone(),
                command: String::new(),
                tier: "each round".into(),
                skipped: Some("the blueprint's checks don't list it".into()),
            });
            continue;
        };
        if check.kind != "machine" {
            continue;
        }
        let skipped = if !check.when.is_empty() && !touched(&check.when) {
            Some(format!(
                "the change touched none of {}",
                check.when.join(" ")
            ))
        } else if id != "seal" && check.command.is_none() {
            Some("it has no command".into())
        } else {
            None
        };
        out.push(Planned {
            check: id.clone(),
            command: check.command.clone().unwrap_or_default(),
            tier: check.runs.clone(),
            skipped,
        });
    }
    // seal first, then each round, then before review, each in the blueprint's order.
    out.sort_by_key(|p| {
        if p.check == "seal" {
            0
        } else if p.tier == "before review" {
            2
        } else {
            1
        }
    });
    out
}

/// The last lines of an output, five at most, without trailing blank lines.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let from = lines.len().saturating_sub(n);
    lines[from..].join("\n")
}

/// The seconds the project's runner said a check took, from its last line
/// `gdd-check {"check":"<id>","seconds":<n>,"exit":<n>}`.
pub fn runner_seconds(text: &str) -> Option<f64> {
    text.lines().rev().find_map(|l| {
        let json = l.trim().strip_prefix("gdd-check ")?;
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        v.get("seconds")?.as_f64()
    })
}

/// The checks of the standing gates of a zone: each `@check:<id>` of its feature.
pub fn zone_gate_checks(feature: &str) -> Vec<String> {
    let mut out = vec![];
    for word in feature.split_whitespace() {
        if let Some(id) = word.strip_prefix("@check:") {
            let id = id.trim_end_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '-'));
            if !id.is_empty() && !out.iter().any(|x| x == id) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// Run one check's command with `sh -c` in a directory, standard input closed, its output to a
/// file, and say what came out.
pub fn run_check(dir: &Path, check: &str, command: &str, tier: &str, output: &Path) -> Ran {
    let started = crate::time::instant();
    let file = crate::runs::create_new(output);
    let status = match file {
        Ok(f) => {
            let err = f.try_clone();
            let mut c = Command::new("sh");
            c.arg("-c")
                .arg(command)
                .current_dir(dir)
                .stdin(Stdio::null())
                .stdout(f);
            match err {
                Ok(e) => {
                    c.stderr(e);
                }
                Err(_) => {
                    c.stderr(Stdio::null());
                }
            }
            c.status()
        }
        Err(e) => Err(e),
    };
    let clock = started.elapsed().as_secs_f64();
    let text = std::fs::read_to_string(output).unwrap_or_default();
    let (exit, extra) = match status {
        Ok(s) => (s.code(), String::new()),
        Err(e) => (None, format!("it could not run: {e}")),
    };
    let seconds = runner_seconds(&text).unwrap_or((clock * 100.0).round() / 100.0);
    let mut t = tail(&text, 5);
    if !extra.is_empty() {
        t = extra;
    }
    Ran {
        check: check.into(),
        command: command.into(),
        tier: tier.into(),
        exit,
        seconds: Some(seconds),
        tail: t,
        passed: exit == Some(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp() -> Blueprint {
        Blueprint::parse(
            "---\nchecks:\n  - id: e2e\n    type: machine\n    command: python3 e2e.py\n    runs: before review\n    when: [':(glob)src/texts/**']\n  - id: tests\n    type: machine\n    command: make test\n  - id: seal\n    type: machine\n    command: \"git diff --exit-code <start> HEAD -- gdd features <sealed paths> ':(exclude)gdd/log'\"\n  - id: review\n    type: reviewer\ncolors:\n  green:\n    checks: [e2e, tests, seal, review]\n---\n",
        )
        .unwrap()
    }

    #[test]
    fn seal_goes_first_and_slow_checks_last() {
        let p = plan(&bp(), "green", &|_| true, &[]);
        let ids: Vec<&str> = p.iter().map(|x| x.check.as_str()).collect();
        assert_eq!(ids, vec!["seal", "tests", "e2e"]);
        let p = plan(&bp(), "green", &|_| false, &[]);
        assert!(p[2].skipped.as_ref().unwrap().contains("touched none"));
    }

    #[test]
    fn the_seal_command_takes_start_and_paths() {
        let c = seal_command(
            &bp(),
            "abc",
            &["tests/t one.py".into(), "gdd/briefs/x.md".into()],
        );
        assert_eq!(
            c,
            "git diff --exit-code abc HEAD -- gdd features 'tests/t one.py' gdd/briefs/x.md ':(exclude)gdd/log'"
        );
        // <start> reaches a shell: it is quoted like the paths.
        let c = seal_command(&bp(), "abc; touch pwned", &[]);
        assert!(
            c.starts_with("git diff --exit-code 'abc; touch pwned' HEAD --"),
            "{c}"
        );
    }

    #[test]
    fn every_command_is_filled_and_a_placeholder_left_is_named() {
        let h = "0123456789abcdef0123456789abcdef01234567";
        let f = Fill {
            start: h,
            base: Some(h),
            sealed: &["tests/a b.sh".into()],
        };
        assert_eq!(
            fill(
                "git rev-parse --verify --quiet \"<start>^{commit}\" > /dev/null && scan --log-opts=\"<start>..HEAD\" <base>",
                &f
            )
            .unwrap(),
            format!(
                "git rev-parse --verify --quiet \"{h}^{{commit}}\" > /dev/null && scan --log-opts=\"{h}..HEAD\" {h}"
            )
        );
        assert_eq!(
            fill("ls <sealed paths> 2>&1 < /dev/null", &f).unwrap(),
            "ls 'tests/a b.sh' 2>&1 < /dev/null"
        );
        assert_eq!(
            fill("x <head>", &f).unwrap_err(),
            "<head>",
            "only what the contract names"
        );
        assert!(fill("<lint command from the constitution>", &f).is_err());
        // Not a hash: <start> stays, and is named.
        let bad = Fill {
            start: "main; rm -rf x",
            base: None,
            sealed: &[],
        };
        assert_eq!(fill("git diff <start>", &bad).unwrap_err(), "<start>");
        assert_eq!(fill("cat <<EOF > out", &f), Ok("cat <<EOF > out".into()));
        assert_eq!(placeholder("sort <in.txt >out.txt"), None);
    }

    #[test]
    fn tails_and_runner_lines() {
        assert_eq!(tail("a\nb\nc\nd\ne\nf\n\n", 5), "b\nc\nd\ne\nf");
        assert_eq!(
            runner_seconds("ok\ngdd-check {\"check\":\"tests\",\"seconds\":7,\"exit\":0}\n"),
            Some(7.0)
        );
        assert_eq!(
            zone_gate_checks(
                "@zone:pay\n  Rule: x\n    @check:prices\n    Scenario: y\n    @check:review\n"
            ),
            vec!["prices", "review"]
        );
    }

    #[test]
    fn runs_a_command() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("o.txt");
        let r = run_check(
            dir.path(),
            "x",
            "echo one; echo two >&2; exit 3",
            "each round",
            &out,
        );
        assert_eq!(r.exit, Some(3));
        assert!(!r.passed);
        assert!(r.tail.contains("one") && r.tail.contains("two"));
    }
}
