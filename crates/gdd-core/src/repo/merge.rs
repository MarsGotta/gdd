//! Merging a change (`gdd merge`): once G3 is open, the last work of the change is committed on
//! its branch, and the branch merges into the main branch with a merge commit that carries the
//! record of the change: what a pull request description would hold. The full working files stay
//! in `.gdd/changes/<id>/`, which git does not keep. The known debt the change paid goes off the
//! list in that same merge commit. Then main is pushed when the rules ask for it and there is a
//! remote, and the artifact is built and deployed to pre: as the main branch configures them
//! once the change merged into it, not as the branch of the change had them.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::change;
use crate::classify::Classification;
use crate::config::{self, Color};
use crate::gates::release::{self, Passed, Steps};
use crate::gates::{self, Gate, approve, debt};
use crate::git::Git;
use crate::guard;
use crate::log;

/// Why a change without G3 does not merge.
const G3_CLOSED: &str = "G3 is not open.";

/// Characters the message keeps of a line it quotes, at most.
const LINE_LENGTH: usize = 220;

/// Decisions taken without the human the message lists: the last ones.
const DECIDED: usize = 8;

/// Agents of the log lines that are no agent calls: their tokens do not count.
const NOT_AGENTS: [&str; 9] = [
    "-",
    "checks",
    "path-guard",
    "merge",
    "verdict",
    "build",
    "deploy_pre",
    "tests_pre",
    "demo",
];

/// What merging a change reads from the project.
#[derive(Debug, Clone, Copy)]
pub struct Project<'a> {
    /// The project root.
    pub root: &'a Path,
    /// The state root, whose `.gdd/` holds the record of the change (see
    /// [`crate::state::root`]).
    pub state: &'a Path,
    /// The branch the change merges into.
    pub main: &'a str,
}

/// Whether a rules file asks to push the main branch after the Merge Gate: `push_main`; no when
/// it says nothing. The rest of the file is ignored.
fn push_main(rules: &str) -> Result<bool, config::Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Raw {
        push_main: bool,
    }
    let raw: Raw = toml::from_str(rules).map_err(|e| config::Error::Toml(e.to_string()))?;
    Ok(raw.push_main)
}

/// Merges change `id`, classified as `classification`, into the main branch (see the module).
/// It needs G3 open and the branch of the change checked out, where its last work is. The known
/// debt its latest G2 and G3 named as paid goes off `gdd/debt.toml` in the merge commit (see
/// [`debt::pay`]); when it cannot, the merge is given up. The record goes to state `merged`
/// with the merge commit, and the merge is logged; then the rules file and the release commands
/// are read as the merge left them, and a configuration that cannot be read, or a failed push,
/// build or deploy, is an error after that. What it prints, as JSON on one line: `ok`, whether
/// the deploy to pre was simulated, and that step.
pub fn merge(
    project: &Project,
    id: &str,
    classification: &Classification,
) -> Result<String, String> {
    let (root, state) = (project.root, project.state);
    let (text, record) = release::read(state, id)?;
    if record["gates"]["G3"]["decision"] != "ok" {
        return Err(G3_CLOSED.to_string());
    }
    let git = Git::new(root);
    let branch = change::branch(id);
    let said = |e: crate::git::Error| e.to_string();
    if git.current_branch().map_err(said)? != branch {
        return Err(format!(
            "Not on branch {branch}: merge runs where the last work of the change is."
        ));
    }
    let paid = debt::reported(state, id)?.paid;
    let names = debt::by_risk(paid.iter().map(|(_, name)| name.as_str()));
    let message = message(state, id, &record, classification, &names)?;
    if git.has_changes().map_err(said)? {
        let work = format!("gdd({id}): change work");
        git.commit_all(&work).map_err(said)?;
    }
    git.checkout(project.main).map_err(said)?;
    if paid.is_empty() {
        git.merge(&branch, &message).map_err(said)?;
    } else {
        merge_paying(&git, root, &branch, &message, &paid)?;
    }
    let commit = git.head().map_err(said)?;
    let path = change::record_path(id);
    let fields = vec![
        ("state", Value::from("merged")),
        ("commit", Value::from(commit.as_str())),
    ];
    let merged = change::with_fields(&text, fields).map_err(|e| format!("{path}: {e}"))?;
    change::write(state, &path, &merged)?;
    let entry = log::Entry {
        change: id.to_string(),
        phase: "3-integrate".to_string(),
        gate: "G3".to_string(),
        role: "machine".to_string(),
        agent: "merge".to_string(),
        result: "ok".to_string(),
        duration_s: 0.0,
        note: commit.chars().take(12).collect(),
    };
    change::logged(state, &entry)?;
    let (push, steps) = configured(root)?;
    if push && git.has_remote().map_err(said)? {
        let main = project.main;
        if let Err(e) = git.push(main) {
            return Err(format!("Merged into {main}, but the push failed: {e}"));
        }
    }
    let build = release::step(root, state, id, &steps, "build")?;
    if !build.ok {
        let why = build.failure();
        return Err(format!("Could not build the artifact: {why}"));
    }
    let pre = release::step(root, state, id, &steps, "deploy_pre")?;
    if !pre.ok {
        let why = pre.failure();
        return Err(format!("Could not deploy to pre: {why}"));
    }
    Passed::detail(&pre, Some(pre.simulated)).json()
}

/// Merges `branch` into the current branch with a merge commit whose message is `message`, and
/// takes the known debt `paid`, each with its gate, off the list of the project at `root` in that
/// same commit. When the list cannot be written or the commit fails, the merge is given up and
/// the current branch stays as it was.
fn merge_paying(
    git: &Git,
    root: &Path,
    branch: &str,
    message: &str,
    paid: &[(Gate, String)],
) -> Result<(), String> {
    let said = |e: crate::git::Error| e.to_string();
    git.merge_uncommitted(branch).map_err(said)?;
    let committed = debt::pay(root, paid).and_then(|()| git.commit_all(message).map_err(said));
    committed.map_err(|e| match git.abort_merge() {
        Ok(()) => format!("{e}. The merge was given up: main is as it was."),
        Err(abort) => format!("{e}. Giving up the merge failed too: {abort}"),
    })
}

/// Whether to push the main branch, and the release commands, as the project at `root` has them
/// in its working tree: after the merge, the main branch's. A missing file says nothing.
fn configured(root: &Path) -> Result<(bool, Steps), String> {
    let rules = configuration(root, guard::RULES)?;
    let push = push_main(&rules).map_err(|e| format!("{}: {e}", guard::RULES))?;
    let commands = configuration(root, gates::COMMANDS)?;
    let steps = Steps::from_toml(&commands).map_err(|e| format!("{}: {e}", gates::COMMANDS))?;
    Ok((push, steps))
}

/// The text of the configuration file `path` of the project at `root`: empty when there is none.
fn configuration(root: &Path, path: &str) -> Result<String, String> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// The message of the merge commit of change `id` of the project whose state root is `state`,
/// whose record is `record` and whose classification is `classification`: the title, then the
/// color, lane and kind of work; the approvals; the verdicts of the Reviewer and its points; the
/// decisions taken without the human (the last ones); the tokens of its agent calls; the known
/// debt it paid, `paid`, when there is; and where its record lives.
pub fn message(
    state: &Path,
    id: &str,
    record: &Value,
    classification: &Classification,
    paid: &[&str],
) -> Result<String, String> {
    let title = record["title"].as_str().unwrap_or_default();
    let kind = record["type"].as_str().unwrap_or("-");
    let (color, lane) = (classification.color, classification.lane);
    let mut lines = vec![
        format!("gdd({id}): G3 Merge Gate · {title}"),
        String::new(),
        format!("Color: {color} · lane {lane} · type {kind}"),
    ];
    let approvals = approvals(state, record, color)?;
    if approvals.is_empty() {
        lines.push("Approvals: none needed".to_string());
    } else {
        lines.push(format!("Approvals: {}", approvals.join("; ")));
    }
    lines.extend(reviews(state, id)?);
    let none = Vec::new();
    let notes = record["inbox"].as_array().unwrap_or(&none);
    let decided: Vec<&str> = notes
        .iter()
        .filter(|note| note["kind"] == "decided")
        .filter_map(|note| note["text"].as_str())
        .collect();
    if !decided.is_empty() {
        lines.push("Decided without the human:".to_string());
        for text in &decided[decided.len().saturating_sub(DECIDED)..] {
            lines.push(format!("  - {}", clipped(&text.replace('\n', " "))));
        }
    }
    lines.extend(tokens(state, id)?);
    if !paid.is_empty() {
        lines.push(format!("Known debt paid: {}", paid.join(", ")));
    }
    lines.push(format!("Record: .gdd/changes/{id}/ (local)"));
    Ok(lines.join("\n"))
}

/// The approvals the message names: the red zones for a red change, then G1 and the plan when
/// the record has them.
fn approvals(state: &Path, record: &Value, color: Color) -> Result<Vec<String>, String> {
    let mut approvals = Vec::new();
    let zones = match color {
        Color::Red => saved(state, approve::ZONES_PATH)?,
        _ => None,
    };
    if let Some(zones) = zones {
        approvals.push(format!("red zones {}", approval(&zones, "")));
    }
    for gate in ["G1", "plan"] {
        let decided = &record["gates"][gate];
        if decided.as_object().map_or(0, Map::len) == 0 {
            continue;
        }
        let sealed = decided["hashes"].as_object().map_or(0, Map::len);
        let extra = match sealed {
            0 => String::new(),
            files => format!(", {files} scenario file(s) sealed"),
        };
        approvals.push(format!("{gate} {}", approval(decided, &extra)));
    }
    Ok(approvals)
}

/// An approval in one line: `<decision> (<by>, <day><extra>)`.
fn approval(decided: &Value, extra: &str) -> String {
    let text = |key: &str, missing: &str| decided[key].as_str().unwrap_or(missing).to_string();
    let day: String = text("ts", "").chars().take(10).collect();
    let (decision, by) = (text("decision", "-"), text("by", "-"));
    format!("{decision} ({by}, {day}{extra})")
}

/// The JSON object a file of the tool under the state root `state` holds: `None` when there is no
/// file, or it holds no object or an empty one.
fn saved(state: &Path, path: &str) -> Result<Option<Value>, String> {
    let text = match fs::read_to_string(state.join(path)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {path}: {e}")),
    };
    let fields: Map<String, Value> = serde_json::from_str(&text).unwrap_or_default();
    Ok((!fields.is_empty()).then_some(Value::Object(fields)))
}

/// The lines of the Reviewer: its verdicts, in the order of its outputs
/// (`.gdd/changes/<id>/outputs/reviewer-*.md`), then the points of each output (its lines that
/// start with `- **`), as plain text. Nothing without outputs.
fn reviews(state: &Path, id: &str) -> Result<Vec<String>, String> {
    let folder = format!(".gdd/changes/{id}/outputs");
    let entries = match fs::read_dir(state.join(&folder)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {folder}: {e}")),
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("reviewer-") && name.ends_with(".md"))
        .collect();
    names.sort();
    let mut outputs = Vec::new();
    for name in names {
        let path = format!("{folder}/{name}");
        let bytes = fs::read(state.join(&path)).map_err(|e| format!("cannot read {path}: {e}"))?;
        outputs.push(String::from_utf8_lossy(&bytes).into_owned());
    }
    if outputs.is_empty() {
        return Ok(Vec::new());
    }
    let verdicts: Vec<&str> = outputs.iter().map(String::as_str).map(verdict).collect();
    let mut lines = vec![format!("Reviewer: {}", verdicts.join(" -> "))];
    for output in &outputs {
        for line in output.lines().filter(|line| line.starts_with("- **")) {
            let point = unlinked(&line[2..]).replace("**", "");
            lines.push(format!("  - {}", clipped(&point)));
        }
    }
    Ok(lines)
}

/// The verdict of an output of the Reviewer: the `OK` or `KO` after the first `VERDICT:` that
/// has one; `?` when none has.
fn verdict(output: &str) -> &'static str {
    output
        .match_indices("VERDICT:")
        .find_map(|(at, mark)| {
            let rest = output[at + mark.len()..].trim_start();
            ["OK", "KO"].into_iter().find(|word| rest.starts_with(word))
        })
        .unwrap_or("?")
}

/// `text` with each Markdown link, `[label](target)` with a label, replaced by its label.
fn unlinked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find(']').filter(|&close| close > 0);
        let link = close.and_then(|close| {
            let target = after[close + 1..].strip_prefix('(')?;
            let end = target.find(')')?;
            Some((&after[..close], &target[end + 1..]))
        });
        match link {
            Some((label, tail)) => {
                out.push_str(label);
                rest = tail;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The tokens of the agent calls of change `id` in the log under the state root `state`, in one
/// line: nothing when there are none.
fn tokens(state: &Path, id: &str) -> Result<Option<String>, String> {
    let text = match fs::read_to_string(state.join(log::PATH)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", log::PATH)),
    };
    let records = log::parse(&text).map_err(|e| format!("{}, {e}", log::PATH))?;
    let change = Value::from(id);
    let calls: Vec<&log::Record> = records
        .iter()
        .filter(|record| record.fields.get("change") == Some(&change))
        .filter(|record| {
            let agent = record.fields.get("agent").and_then(Value::as_str);
            !agent.is_some_and(|agent| NOT_AGENTS.contains(&agent))
        })
        .collect();
    if calls.is_empty() {
        return Ok(None);
    }
    let sum = |tokens: fn(&log::Record) -> u64| calls.iter().copied().map(tokens).sum::<u64>();
    let cost: f64 = calls.iter().map(|record| record.cost_usd).sum();
    let line = format!(
        "Tokens: in {} · cache {} · out {} · {cost:.2} USD (estimate) · {} agent call(s)",
        grouped(sum(|r| r.tokens_in)),
        grouped(sum(|r| r.tokens_cache)),
        grouped(sum(|r| r.tokens_out)),
        calls.len()
    );
    Ok(Some(line))
}

/// `n` with a comma between each group of three digits: `1,234,567`.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `text` to [`LINE_LENGTH`] characters at most.
fn clipped(text: &str) -> String {
    text.chars().take(LINE_LENGTH).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{Change, classify};
    use crate::git::tests::{Folder, Repo};
    use crate::rules;
    use crate::zones::ZoneMap;

    const ZONES: &str = "default = \"yellow\"\n[[zone]]\ncolor = \"red\"\n\
                         paths = [\"src/storage/**\"]\nreason = \"Drafts\"\n";

    const COMMANDS: &str = "[release]\nsimulated = true\nbuild = \"true\"\n";

    fn classified(path: &str) -> Classification {
        let change = Change {
            paths: vec![path.to_string()],
            min_color: None,
            resumable: false,
        };
        let zones = ZoneMap::from_toml(ZONES).unwrap();
        classify(&zones, &rules::default_table(), &change).unwrap()
    }

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    /// A line of the log: a call of `agent` for change `change`, with its tokens (in, cache and
    /// out) and its cost.
    fn call(change: &str, agent: &str, tokens: [u64; 3], cost: f64) -> String {
        let line = serde_json::json!({
            "change": change, "agent": agent, "tokens_in": tokens[0],
            "tokens_cache": tokens[1], "tokens_out": tokens[2], "cost_usd": cost,
        });
        line.to_string()
    }

    #[test]
    fn the_rules_say_whether_main_is_pushed() {
        assert_eq!(push_main(""), Ok(false));
        let rules = "main_branch = \"main\"\npush_main = false\n";
        assert_eq!(push_main(rules), Ok(false));
        assert_eq!(push_main("push_main = true\n[limits]\nx = 1\n"), Ok(true));
        for wrong in ["push_main = \"yes\"\n", "push_main = "] {
            let result = push_main(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn a_change_without_approvals_says_none_were_needed() {
        let folder = Folder::new();
        let record = r#"{"title": "App", "type": "Feature", "gates": {"G3": {"decision": "ok"}}}"#;
        let record = json(record);
        let text = message(&folder.0, "c1", &record, &classified("src/app.rs"), &[]).unwrap();
        let expected = "gdd(c1): G3 Merge Gate · App\n\n\
                        Color: yellow · lane full · type Feature\n\
                        Approvals: none needed\n\
                        Record: .gdd/changes/c1/ (local)";
        assert_eq!(text, expected);
        let paid = ["security", "lint"];
        let text = message(&folder.0, "c1", &record, &classified("src/app.rs"), &paid).unwrap();
        let line = "\nApprovals: none needed\nKnown debt paid: security, lint\nRecord: ";
        assert!(text.contains(line), "{text}");
        let app = classified("src/app.rs");
        let bare = message(&folder.0, "c1", &json("{}"), &app, &[]).unwrap();
        let start = "gdd(c1): G3 Merge Gate · \n\nColor: yellow · lane full · type -\n";
        assert!(bare.starts_with(start), "{bare}");
    }

    #[test]
    fn the_message_carries_the_record_of_the_change() {
        let folder = Folder::new();
        let zones = r#"{"decision": "ok", "by": "human", "ts": "2026-10-20T09:00:00+00:00"}"#;
        folder.write(approve::ZONES_PATH, zones);
        let first = "Notes\n- **Blocking** none\nVERDICT: KO\n\
                     - **Fix** [the guard](src/a.rs#L3) now\n";
        let outputs = ".gdd/changes/c1/outputs";
        folder.write(&format!("{outputs}/reviewer-1.md"), first);
        folder.write(&format!("{outputs}/reviewer-2.md"), "VERDICT:\n  OK\n");
        folder.write(&format!("{outputs}/reviewer-3.md"), "no verdict\n");
        let builder = "VERDICT: OK\n- **Not** a review\n";
        folder.write(&format!("{outputs}/builder-1.md"), builder);
        let log = [
            call("c1", "claude", [1200, 3000, 50], 0.5),
            call("c1", "codex", [300, 0, 7], 0.256),
            call("c1", "checks", [999_999, 0, 0], 0.0),
            call("c2", "claude", [999_999, 0, 0], 0.0),
        ];
        folder.write(log::PATH, &log.join("\n"));
        let note = |n: u32| {
            let text = format!("Choice {n}\nwith reason");
            serde_json::json!({"kind": "decided", "text": text})
        };
        let mut inbox = vec![serde_json::json!({"kind": "needs", "text": "Not this"})];
        inbox.extend((1..=9).map(note));
        let record = serde_json::json!({
            "title": "Save",
            "type": "Feature",
            "gates": {
                "G1": {
                    "decision": "ok", "by": "human", "ts": "2026-10-21T10:00:00+00:00",
                    "hashes": {"features/a.feature": "ab", "features/hidden/a.feature": "cd"}
                },
                "plan": {"decision": "ok", "by": "human", "ts": "2026-10-22T11:00:00+00:00"},
                "G3": {"decision": "ok", "by": "machines"}
            },
            "inbox": inbox
        });
        let red = classified("src/storage/db.rs");
        let text = message(&folder.0, "c1", &record, &red, &[]).unwrap();
        let approvals = "Approvals: red zones ok (human, 2026-10-20); \
                         G1 ok (human, 2026-10-21, 2 scenario file(s) sealed); \
                         plan ok (human, 2026-10-22)";
        let tokens = "Tokens: in 1,500 · cache 3,000 · out 57 · \
                      0.76 USD (estimate) · 2 agent call(s)";
        let mut expected = vec![
            "gdd(c1): G3 Merge Gate · Save".to_string(),
            String::new(),
            "Color: red · lane full · type Feature".to_string(),
            approvals.to_string(),
            "Reviewer: KO -> OK -> ?".to_string(),
            "  - Blocking none".to_string(),
            "  - Fix the guard now".to_string(),
            "Decided without the human:".to_string(),
        ];
        expected.extend((2..=9).map(|n| format!("  - Choice {n} with reason")));
        expected.push(tokens.to_string());
        expected.push("Record: .gdd/changes/c1/ (local)".to_string());
        assert_eq!(text, expected.join("\n"));
        // Only a red change names the red zones.
        let yellow = message(&folder.0, "c1", &record, &classified("src/app.rs"), &[]).unwrap();
        let g1 = "\nApprovals: G1 ok (human, 2026-10-21, 2 scenario file(s) sealed); plan";
        assert!(yellow.contains(g1), "{yellow}");
    }

    #[test]
    fn approvals_say_what_they_have() {
        let folder = Folder::new();
        let red = classified("src/storage/db.rs");
        // A red change whose zones nobody approved names no approval of them.
        let text = message(&folder.0, "c1", &json("{}"), &red, &[]).unwrap();
        assert!(text.contains("\nApprovals: none needed\n"), "{text}");
        folder.write(approve::ZONES_PATH, "{}");
        let record = json(r#"{"gates": {"G1": {"decision": "ko"}, "plan": {}}}"#);
        let text = message(&folder.0, "c1", &record, &red, &[]).unwrap();
        assert!(text.contains("\nApprovals: G1 ko (-, )\n"), "{text}");
        folder.write(approve::ZONES_PATH, "{not json");
        let text = message(&folder.0, "c1", &json("{}"), &red, &[]).unwrap();
        assert!(text.contains("\nApprovals: none needed\n"), "{text}");
        fs::remove_file(folder.0.join(approve::ZONES_PATH)).unwrap();
        fs::create_dir_all(folder.0.join(approve::ZONES_PATH)).unwrap();
        let err = message(&folder.0, "c1", &json("{}"), &red, &[]).unwrap_err();
        assert!(err.starts_with("cannot read .gdd/zones.json: "), "{err}");
    }

    #[test]
    fn what_the_message_cannot_read_is_an_error() {
        let folder = Folder::new();
        let yellow = classified("src/app.rs");
        folder.write(log::PATH, "{not json\n");
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[]).unwrap_err();
        assert!(err.starts_with(".gdd/log.jsonl, line 1: "), "{err}");
        fs::remove_file(folder.0.join(log::PATH)).unwrap();
        fs::create_dir_all(folder.0.join(log::PATH)).unwrap();
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[]).unwrap_err();
        assert!(err.starts_with("cannot read .gdd/log.jsonl: "), "{err}");
        fs::remove_dir(folder.0.join(log::PATH)).unwrap();
        folder.write(".gdd/changes/c1/outputs", "not a folder");
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[]).unwrap_err();
        let start = "cannot read .gdd/changes/c1/outputs: ";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(folder.0.join(".gdd/changes/c1/outputs")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/outputs/reviewer-1.md")).unwrap();
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[]).unwrap_err();
        let start = "cannot read .gdd/changes/c1/outputs/reviewer-1.md: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn verdicts_links_and_numbers_are_read_as_the_kit_reads_them() {
        assert_eq!(verdict("VERDICT: OK"), "OK");
        assert_eq!(verdict("x VERDICT:KO y"), "KO");
        assert_eq!(verdict("VERDICT: maybe\nVERDICT: \t OK"), "OK");
        assert_eq!(verdict("verdict: OK"), "?");
        assert_eq!(verdict(""), "?");
        for (text, plain) in [
            ("[a](b)", "a"),
            ("x [the guard](a.rs) y [b]()", "x the guard y b"),
            ("[a [b](c)", "a [b"),
            ("[a] [b](c)", "[a] b"),
            ("[](x) [a](b", "[](x) [a](b"),
            ("[a]x(b)", "[a]x(b)"),
            ("no links", "no links"),
        ] {
            assert_eq!(unlinked(text), plain, "{text}");
        }
        let numbers = [(0, "0"), (999, "999"), (1_000, "1,000")];
        for (n, text) in numbers {
            assert_eq!(grouped(n), text);
        }
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(clipped(&"é".repeat(300)), "é".repeat(220));
        assert_eq!(clipped("short"), "short");
    }

    /// A project in a git repository with an author of its own, its files and its release
    /// commands committed on `main`, and change `c1` started on its branch, with G3 open.
    fn started(ignore: &str) -> Repo {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ignore);
        repo.write("src/old.rs", "old\n");
        repo.write(gates::COMMANDS, COMMANDS);
        repo.commit();
        repo.start("c1");
        let record = r#"{"id": "c1", "title": "App", "type": "Feature", "state": "building",
                         "gates": {"G3": {"decision": "ok", "by": "machines"}},
                         "inbox": [], "x": 1}"#;
        repo.write(".gdd/changes/c1/change.json", record);
        repo
    }

    fn merged(repo: &Repo) -> Result<String, String> {
        let project = Project {
            root: repo.root(),
            state: repo.root(),
            main: "main",
        };
        merge(&project, "c1", &classified("src/app.rs"))
    }

    fn record(repo: &Repo) -> Value {
        json(&change::read_record(repo.root(), "c1").unwrap())
    }

    fn agents(repo: &Repo) -> Vec<String> {
        let text = fs::read_to_string(repo.root().join(log::PATH)).unwrap();
        let records = log::parse(&text).unwrap();
        records.iter().map(|r| r.text("agent")).collect()
    }

    #[test]
    fn merge_keeps_the_last_work_and_records_the_change_in_the_merge_commit() {
        let repo = started(".gdd/*\n");
        repo.write("src/app.rs", "fn main() {}\n");
        let out = json(&merged(&repo).unwrap());
        assert_eq!(out["ok"], true);
        assert_eq!(out["simulated"], true);
        assert_eq!(out["detail"]["name"], "deploy_pre");
        assert_eq!(repo.git(&["branch", "--show-current"]), "main");
        repo.git(&["cat-file", "-e", "main:src/app.rs"]);
        let work = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(work, "gdd(c1): change work");
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        let expected = "gdd(c1): G3 Merge Gate · App\n\nColor: yellow · lane full · type Feature";
        assert!(message.starts_with(expected), "{message}");
        let parents = repo.git(&["log", "-1", "--format=%P", "main"]);
        assert_eq!(parents.split(' ').count(), 2, "a merge commit");
        let record = record(&repo);
        assert_eq!(record["state"], "merged");
        let head = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(record["commit"], head.as_str());
        assert_eq!(record["x"], 1);
        assert_eq!(agents(&repo), ["merge", "build", "deploy_pre"]);
        let text = fs::read_to_string(repo.root().join(log::PATH)).unwrap();
        let line = &log::parse(&text).unwrap()[0];
        assert_eq!(line.text("gate"), "G3");
        assert_eq!(line.text("phase"), "3-integrate");
        assert_eq!(line.text("note"), head[..12]);
    }

    #[test]
    fn the_record_and_the_log_of_a_merge_live_under_the_state_root() {
        let repo = started(".gdd/*\n");
        let state = Folder::new();
        fs::rename(repo.root().join(".gdd"), state.0.join(".gdd")).unwrap();
        repo.write("src/app.rs", "x\n");
        let project = Project {
            root: repo.root(),
            state: &state.0,
            main: "main",
        };
        assert!(merge(&project, "c1", &classified("src/app.rs")).is_ok());
        repo.git(&["cat-file", "-e", "main:src/app.rs"]);
        let record = json(&change::read_record(&state.0, "c1").unwrap());
        assert_eq!(record["state"], "merged");
        let text = fs::read_to_string(state.0.join(log::PATH)).unwrap();
        assert_eq!(log::parse(&text).unwrap().len(), 3);
        assert!(!repo.root().join(".gdd").exists());
    }

    #[test]
    fn a_branch_with_nothing_left_to_commit_merges_as_it_is() {
        let repo = started(".gdd/*\n");
        repo.write("src/app.rs", "x\n");
        repo.commit();
        let tip = repo.git(&["rev-parse", "HEAD"]);
        assert!(merged(&repo).is_ok());
        assert_eq!(repo.git(&["rev-parse", "change/c1"]), tip);
        assert_eq!(repo.git(&["rev-parse", "HEAD^2"]), tip);
    }

    #[test]
    fn local_state_that_git_does_not_ignore_merges_with_the_change() {
        let repo = started("");
        repo.write("src/app.rs", "x\n");
        assert!(merged(&repo).is_ok());
        assert_eq!(record(&repo)["state"], "merged");
        assert_eq!(agents(&repo), ["merge", "build", "deploy_pre"]);
    }

    /// Known debt of `lint` in G2 and in G3, and of `security` in G3.
    const DEBT: &str = "adopted = \"2026-10-27\"\n\
                        [[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-27\"\n\
                        [[debt]]\ncommand = \"lint\"\ngate = \"g3\"\nsince = \"2026-10-27\"\n\
                        [[debt]]\ncommand = \"security\"\ngate = \"g3\"\nsince = \"2026-10-27\"\n";

    /// The project of [`started`] with `known` as its known debt on `main` before the change
    /// started, the latest G2 of `c1` naming `paid` as paid, and work of the change to commit.
    fn paying(known: &str, paid: &str) -> Repo {
        let repo = started(".gdd/*\n");
        repo.git(&["checkout", "-q", "main"]);
        repo.write(debt::PATH, known);
        repo.commit();
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.git(&["merge", "-q", "main"]);
        let g2 = format!(r#"{{"ok": true, "paid_debt": {paid}}}"#);
        repo.write(".gdd/changes/c1/g2.json", &g2);
        repo.write("src/app.rs", "x\n");
        repo
    }

    #[test]
    fn the_merge_commit_takes_the_paid_debt_off_the_list() {
        let repo = paying(DEBT, r#"["lint"]"#);
        let g3 = r#"{"ok": true, "paid_debt": ["security"]}"#;
        repo.write(".gdd/changes/c1/g3.json", g3);
        assert!(merged(&repo).is_ok());
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        let paid = "\nKnown debt paid: security, lint\n";
        assert!(message.contains(paid), "{message}");
        let parents = repo.git(&["log", "-1", "--format=%P", "main"]);
        assert_eq!(parents.split(' ').count(), 2, "a merge commit");
        // Only the debt of the gate that paid it goes: lint stays in G3.
        let left = repo.git(&["show", "main:gdd/debt.toml"]);
        assert_eq!(left.matches("[[debt]]").count(), 1, "{left}");
        let lint = "command = \"lint\"\ngate = \"g3\"";
        assert!(left.contains(lint), "{left}");
        // The branch of the change keeps the list as it was: the merge commit pays.
        let branch = repo.git(&["show", "change/c1:gdd/debt.toml"]);
        assert_eq!(branch, DEBT.trim_end());
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert_eq!(record(&repo)["state"], "merged");
    }

    #[test]
    fn the_last_debt_paid_takes_the_list_away() {
        let only = "adopted = \"2026-10-27\"\n\
                    [[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-27\"\n";
        let repo = paying(only, r#"["lint"]"#);
        assert!(merged(&repo).is_ok());
        assert!(!repo.root().join(debt::PATH).exists());
        let listed = repo.git(&["ls-tree", "--name-only", "main", "gdd/"]);
        assert_eq!(listed, gates::COMMANDS);
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        assert!(message.contains("\nKnown debt paid: lint\n"), "{message}");
    }

    #[test]
    fn a_debt_list_that_cannot_be_paid_gives_the_merge_up() {
        let repo = paying("[[debt]", "\"lint\"");
        let err = merged(&repo).unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/g2.json: "), "{err}");
        assert_eq!(repo.git(&["branch", "--show-current"]), "change/c1");
        repo.write(".gdd/changes/c1/g2.json", r#"{"paid_debt": ["lint"]}"#);
        let main = repo.git(&["rev-parse", "main"]);
        let err = merged(&repo).unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: invalid TOML: "), "{err}");
        let given_up = "The merge was given up: main is as it was.";
        assert!(err.ends_with(given_up), "{err}");
        assert_eq!(repo.git(&["rev-parse", "main"]), main);
        assert!(!repo.tool().exists("MERGE_HEAD").unwrap());
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert_eq!(record(&repo)["state"], "building");
    }

    #[test]
    fn merge_needs_g3_and_the_branch_of_the_change() {
        let repo = started(".gdd/*\n");
        let main = repo.git(&["rev-parse", "main"]);
        let closed: Result<String, String> = Err(G3_CLOSED.to_string());
        for gates in ["{}", r#"{"G3": {"decision": "ko"}}"#, r#"{"G3": "ok"}"#] {
            let record = format!(r#"{{"id": "c1", "gates": {gates}}}"#);
            repo.write(".gdd/changes/c1/change.json", &record);
            assert_eq!(merged(&repo), closed, "{gates}");
        }
        assert_eq!(G3_CLOSED, "G3 is not open.");
        let open = r#"{"gates": {"G3": {"decision": "ok"}}}"#;
        repo.write(".gdd/changes/c1/change.json", open);
        repo.git(&["checkout", "-q", "main"]);
        repo.write("src/loose.rs", "x\n");
        let expected = "Not on branch change/c1: merge runs where the last work of the change is.";
        assert_eq!(merged(&repo), Err(expected.to_string()));
        assert_eq!(repo.git(&["rev-parse", "main"]), main);
        assert!(!repo.root().join(log::PATH).exists());
        let missing = Folder::new();
        let project = Project {
            root: &missing.0,
            state: &missing.0,
            main: "main",
        };
        let err = merge(&project, "c1", &classified("src/app.rs")).unwrap_err();
        assert!(err.starts_with("change c1 does not exist"), "{err}");
    }

    #[test]
    fn a_failed_build_or_deploy_fails_the_merge_after_it_is_recorded() {
        let repo = started(".gdd/*\n");
        let broken = "[release]\nsimulated = true\nbuild = \"echo no; exit 3\"\n";
        repo.write(gates::COMMANDS, broken);
        let err = merged(&repo).unwrap_err();
        let expected = "Could not build the artifact: release.build failed (exit code 3):\nno";
        assert_eq!(err, expected);
        assert_eq!(record(&repo)["state"], "merged");
        assert_eq!(agents(&repo), ["merge", "build"]);
        let repo = started(".gdd/*\n");
        repo.write(gates::COMMANDS, "[release]\nbuild = \"true\"\n");
        let err = merged(&repo).unwrap_err();
        let expected = "Could not deploy to pre: release.deploy_pre failed:\n\
                        release.deploy_pre not configured and simulated = false";
        assert_eq!(err, expected);
    }

    /// A bare repository in a temporary folder, the remote `origin` of `repo`.
    fn remote(repo: &Repo) -> Folder {
        let remote = Folder::new();
        let path = remote.0.to_string_lossy().into_owned();
        repo.git(&["init", "-q", "--bare", &path]);
        repo.git(&["remote", "add", "origin", &path]);
        remote
    }

    #[test]
    fn main_is_pushed_only_when_the_rules_ask_and_there_is_a_remote() {
        let asked = "push_main = true\n";
        let repo = started(".gdd/*\n");
        repo.write(guard::RULES, asked);
        assert!(merged(&repo).is_ok(), "no remote");
        let repo = started(".gdd/*\n");
        let remote = remote(&repo);
        assert!(merged(&repo).is_ok());
        let origin = Git::new(&remote.0);
        assert!(!origin.exists("main").unwrap(), "not asked to push");
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write(guard::RULES, asked);
        assert!(merged(&repo).is_ok());
        let pushed = origin.merge_base("main", "main").unwrap();
        assert_eq!(pushed, repo.git(&["rev-parse", "main"]));
        let repo = started(".gdd/*\n");
        repo.git(&["remote", "add", "origin", "/nonexistent/gdd-remote"]);
        repo.write(guard::RULES, asked);
        let err = merged(&repo).unwrap_err();
        let start = "Merged into main, but the push failed: ";
        assert!(err.starts_with(start), "{err}");
        assert_eq!(record(&repo)["state"], "merged");
    }

    #[test]
    fn what_follows_the_merge_runs_as_main_configures_it() {
        let repo = started(".gdd/*\n");
        let remote = remote(&repo);
        // The main branch changed its release after the branch of the change left it.
        repo.git(&["checkout", "-q", "main"]);
        let real = "[release]\nbuild = \"true\"\ndeploy_pre = \"touch deployed\"\n";
        repo.write(gates::COMMANDS, real);
        repo.write(guard::RULES, "push_main = true\n");
        repo.commit();
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write("src/app.rs", "x\n");
        let out = json(&merged(&repo).unwrap());
        assert_eq!(out["simulated"], false);
        assert!(repo.root().join("deployed").exists());
        let pushed = Git::new(&remote.0).merge_base("main", "main").unwrap();
        assert_eq!(pushed, repo.git(&["rev-parse", "main"]));
    }

    #[test]
    fn a_configuration_that_cannot_be_read_fails_the_merge_after_it_is_recorded() {
        let failed = |path: &str, text: &str| {
            let repo = started(".gdd/*\n");
            repo.write(path, text);
            let err = merged(&repo).unwrap_err();
            assert_eq!(record(&repo)["state"], "merged", "{path}");
            assert_eq!(agents(&repo), ["merge"], "{path}");
            err
        };
        let err = failed(gates::COMMANDS, "[release\n");
        let start = "gdd/commands.toml: invalid TOML";
        assert!(err.starts_with(start), "{err}");
        let err = failed(guard::RULES, "push_main = \"yes\"\n");
        let start = "gdd/rules.toml: invalid TOML";
        assert!(err.starts_with(start), "{err}");
        // A folder where the rules file should be.
        let err = failed("gdd/rules.toml/x", "");
        let start = "cannot read gdd/rules.toml: ";
        assert!(err.starts_with(start), "{err}");
    }
}
