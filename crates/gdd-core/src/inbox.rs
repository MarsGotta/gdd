//! The Decision Inbox of a change: what the human must decide, what was decided without them
//! and what was parked, from the notes of its record; the security exceptions its last G3 named
//! as added; and the known debt its latest G2 and G3 found still failing or paid.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::change::{self, NOTE_KINDS, POLISH};
use crate::gates::{Gate, debt};

/// The title of the group of each kind of note, in the order of [`NOTE_KINDS`].
const HEADINGS: [&str; 3] = ["Needs your decision", "Decided without you", "Parked"];

/// The title of the group of the security exceptions the change adds.
const EXCEPTIONS: &str = "Security exceptions this change adds";

/// The title of the group of the commands with known debt still failing.
const KNOWN_DEBT: &str = "Known debt";

/// The title of the group of the commands with known debt the change paid.
const DEBT_PAID: &str = "Known debt paid";

/// A group of the inbox from the gate reports: its title and one item per entry.
pub type Group = (&'static str, Vec<String>);

/// What the paths outside the package that the scope check accepted say among the decisions.
const ACCEPTED: &str = "Paths outside the package, same or lower color, accepted by rule:";

/// What follows the number of requests resolved before the last launch.
const RESOLVED: &str =
    "earlier request(s) were resolved before the last launch (kept in change.json).";

#[derive(Default, Deserialize)]
#[serde(default)]
struct Raw {
    title: Option<String>,
    state: Option<String>,
    decision: Option<RawDecision>,
    inbox: Option<Vec<RawNote>>,
    out_of_scope: Option<Vec<String>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawDecision {
    color: Option<String>,
    lane: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawNote {
    kind: String,
    text: String,
    resolved: Value,
}

/// The Decision Inbox of change `id`, from the text of its record and `reported`, the groups
/// from its gate reports (see [`save`]), without a line break at the end: its title, color, lane
/// and state, then one group per kind of note with the notes still open (`Nothing.` when there
/// are none) and, among the decisions, the paths outside the package the scope check accepted;
/// then each group of `reported` that has entries; then the human's notes at G4, when there are,
/// under `Polish`; last, how many requests were resolved before the last launch, if any. Notes
/// of other kinds are left out.
pub fn render(id: &str, record: &str, reported: &[Group]) -> Result<String, serde_json::Error> {
    let raw: Raw = serde_json::from_str(record)?;
    let title = raw.title.unwrap_or_default();
    let decision = raw.decision.unwrap_or_default();
    let color = decision.color.unwrap_or_else(|| "?".into());
    let lane = decision.lane.unwrap_or_else(|| "?".into());
    let state = raw.state.unwrap_or_else(|| "?".into());
    let mut groups: [Vec<String>; 3] = Default::default();
    let mut polish = Vec::new();
    let mut resolved = 0;
    for note in raw.inbox.unwrap_or_default() {
        let group = NOTE_KINDS.iter().position(|kind| *kind == note.kind);
        if is_resolved(&note.resolved) {
            resolved += 1;
        } else if let Some(group) = group {
            groups[group].push(note.text);
        } else if note.kind == POLISH {
            polish.push(note.text);
        }
    }
    let accepted = raw.out_of_scope.unwrap_or_default();
    if !accepted.is_empty() {
        groups[1].push(format!("{ACCEPTED} {}", accepted.join(", ")));
    }
    let mut blocks = vec![
        format!("# Decision Inbox · {id} · {title}"),
        format!("Color {color} · lane {lane} · state {state}"),
    ];
    for (heading, notes) in HEADINGS.iter().zip(groups) {
        let mut block = format!("## {heading}");
        if notes.is_empty() {
            block.push_str("\n- Nothing.");
        }
        for note in notes {
            block.push_str(&format!("\n- {note}"));
        }
        blocks.push(block);
    }
    for (heading, lines) in reported {
        blocks.extend(listed(heading, lines));
    }
    blocks.extend(listed("Polish", &polish));
    if resolved > 0 {
        blocks.push(format!("_{resolved} {RESOLVED}_"));
    }
    Ok(blocks.join("\n\n"))
}

/// The group `heading` with one item per line of `lines`; nothing without lines.
fn listed(heading: &str, lines: &[String]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let items: Vec<String> = lines.iter().map(|line| format!("- {line}")).collect();
    Some(format!("## {heading}\n{}", items.join("\n")))
}

/// Renders the Decision Inbox of change `id` of the project whose state root is `state` from the
/// text of its record (see [`render`]) and its gate reports: the security exceptions its last G3
/// named as added, then the commands with known debt its latest G2 and G3 found still failing,
/// with what their output keeps, then the ones they found paid (see [`debt::reported`]). It
/// saves it next to the record, with a line break at the end. What it rendered.
pub fn save(state: &Path, id: &str, record: &str) -> Result<String, String> {
    let added = added_exceptions(state, id)?;
    let known = debt::reported(state, id)?;
    let reported = [
        (EXCEPTIONS, added),
        (KNOWN_DEBT, known.owed.iter().map(still_failing).collect()),
        (DEBT_PAID, known.paid.iter().map(paid_in).collect()),
    ];
    let rendered =
        render(id, record, &reported).map_err(|e| format!("{}: {e}", change::record_path(id)))?;
    change::write(state, &change::inbox_path(id), &format!("{rendered}\n"))?;
    Ok(rendered)
}

/// The entry of a command with known debt still failing in its gate: its name and gate, what it
/// reported against the count of its debt when it has one, then what its output keeps, as a
/// block of code inside the entry.
fn still_failing((gate, owed): &(Gate, debt::Owed)) -> String {
    let lines = owed.tail.lines().map(|line| format!("\n  {line}"));
    let tail: String = lines.collect();
    let counted = match (owed.count, owed.known) {
        (Some(count), Some(known)) => format!(", reports {count} (the known debt is {known})"),
        _ => String::new(),
    };
    let (name, code) = (&owed.name, format!("  ```{tail}\n  ```"));
    format!("{name} ({gate}){counted}, still failing:\n{code}")
}

/// The entry of a command with known debt paid in its gate: its name and gate.
fn paid_in((gate, name): &(Gate, String)) -> String {
    format!("{name} ({gate})")
}

/// The security exceptions the last G3 of change `id` of the project whose state root is `state`
/// named as added, from its report: none before G3 ran.
fn added_exceptions(state: &Path, id: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Saved {
        #[serde(default)]
        added_exceptions: Vec<String>,
    }
    let path = Gate::G3.report_path(id);
    let text = match fs::read_to_string(state.join(&path)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {path}: {e}")),
    };
    let saved: Saved = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(saved.added_exceptions)
}

/// Whether a note was resolved: its `resolved` says when, or says so.
pub(crate) fn is_resolved(resolved: &Value) -> bool {
    match resolved {
        Value::Null | Value::Bool(false) => false,
        Value::String(when) => !when.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;

    const NONE: [Group; 0] = [];

    const RECORD: &str = r#"{
        "id": "c1", "title": "Drafts", "state": "building",
        "decision": {"color": "red", "lane": "full"},
        "inbox": [
            {"kind": "needs", "text": "Old question", "resolved": "2026-01-02"},
            {"kind": "needs", "text": "Pick a name", "ts": "2026-01-03T00:00:00+00:00"},
            {"kind": "decided", "text": "Kept the old format"},
            {"kind": "other", "text": "Not shown"},
            {"kind": "decided", "text": "Took the short path", "resolved": null}
        ],
        "out_of_scope": ["docs/a.md", "docs/b.md"]
    }"#;

    #[test]
    fn the_inbox_groups_the_open_notes_by_kind() {
        let expected = concat!(
            "# Decision Inbox · c1 · Drafts\n\n",
            "Color red · lane full · state building\n\n",
            "## Needs your decision\n- Pick a name\n\n",
            "## Decided without you\n- Kept the old format\n- Took the short path\n",
            "- Paths outside the package, same or lower color, accepted by rule: ",
            "docs/a.md, docs/b.md\n\n",
            "## Parked\n- Nothing.\n\n",
            "_1 earlier request(s) were resolved before the last launch (kept in change.json)._"
        );
        assert_eq!(render("c1", RECORD, &NONE).unwrap(), expected);
    }

    #[test]
    fn a_record_that_says_little_gets_an_inbox_with_nothing() {
        let expected = concat!(
            "# Decision Inbox · c2 · \n\n",
            "Color ? · lane ? · state ?\n\n",
            "## Needs your decision\n- Nothing.\n\n",
            "## Decided without you\n- Nothing.\n\n",
            "## Parked\n- Nothing."
        );
        assert_eq!(render("c2", "{}", &NONE).unwrap(), expected);
        let nulls = r#"{"decision": null, "inbox": null, "out_of_scope": null}"#;
        assert_eq!(render("c2", nulls, &NONE).unwrap(), expected);
        let empty = r#"{"decision": {}, "inbox": [], "out_of_scope": []}"#;
        assert_eq!(render("c2", empty, &NONE).unwrap(), expected);
    }

    #[test]
    fn parked_notes_have_their_group() {
        let record = r#"{"inbox": [{"kind": "parked", "text": "Waiting for the API"}]}"#;
        let text = render("c1", record, &NONE).unwrap();
        let end = "## Parked\n- Waiting for the API";
        assert!(text.ends_with(end), "{text}");
    }

    #[test]
    fn the_human_notes_at_g4_have_their_group_only_when_there_are() {
        let record = r#"{"inbox": [
            {"kind": "polish", "text": "Shorter errors"},
            {"kind": "parked", "text": "Waiting"},
            {"kind": "polish", "text": "Old", "resolved": "2026-01-02"},
            {"kind": "polish", "text": "Plainer words"}
        ]}"#;
        let text = render("c1", record, &NONE).unwrap();
        let end = concat!(
            "## Parked\n- Waiting\n\n",
            "## Polish\n- Shorter errors\n- Plainer words\n\n",
            "_1 earlier request(s) were resolved before the last launch (kept in change.json)._"
        );
        assert!(text.ends_with(end), "{text}");
        assert!(!render("c1", "{}", &NONE).unwrap().contains("Polish"));
    }

    #[test]
    fn the_exceptions_the_change_adds_have_their_group_only_when_there_are() {
        let record = r#"{"inbox": [{"kind": "polish", "text": "Shorter errors"}]}"#;
        let line = "exception added: A · deny.toml · no end date · r";
        let added = (EXCEPTIONS, vec![line.to_string()]);
        let text = render("c1", record, &[added, (KNOWN_DEBT, vec![])]).unwrap();
        let end = concat!(
            "## Parked\n- Nothing.\n\n",
            "## Security exceptions this change adds\n",
            "- exception added: A · deny.toml · no end date · r\n\n",
            "## Polish\n- Shorter errors"
        );
        assert!(text.ends_with(end), "{text}");
        let text = render("c1", record, &NONE).unwrap();
        assert!(!text.contains("Security exceptions"), "{text}");
    }

    #[test]
    fn the_saved_inbox_shows_the_known_debt_still_failing_and_paid() {
        let folder = Folder::new();
        let g2 = r#"{"known_debt": [{"name": "lint", "tail": "unused import\nin a.rs\n"},
                     {"name": "types", "tail": "x", "count": 3, "known": 5},
                     {"name": "quality", "tail": "y", "count": 3}],
                     "paid_debt": ["unit"]}"#;
        folder.write(".gdd/changes/c1/g2.json", g2);
        let g3 = r#"{"added_exceptions": ["exception added: A"], "paid_debt": ["security"]}"#;
        folder.write(".gdd/changes/c1/g3.json", g3);
        let saved = save(&folder.0, "c1", "{}").unwrap();
        let end = concat!(
            "## Parked\n- Nothing.\n\n",
            "## Security exceptions this change adds\n- exception added: A\n\n",
            "## Known debt\n- lint (G2), still failing:\n",
            "  ```\n  unused import\n  in a.rs\n  ```\n",
            "- types (G2), reports 3 (the known debt is 5), still failing:\n  ```\n  x\n  ```\n",
            "- quality (G2), still failing:\n  ```\n  y\n  ```\n\n",
            "## Known debt paid\n- unit (G2)\n- security (G3)"
        );
        assert!(saved.ends_with(end), "{saved}");
        folder.write(".gdd/changes/c1/g2.json", "{not json");
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/g2.json: "), "{err}");
    }

    #[test]
    fn the_saved_inbox_shows_the_exceptions_the_last_g3_named_as_added() {
        let folder = Folder::new();
        let report = ".gdd/changes/c1/g3.json";
        let added = r#"{"ok": true, "added_exceptions": ["exception added: A", "added: B"]}"#;
        folder.write(report, added);
        let saved = save(&folder.0, "c1", "{}").unwrap();
        let group = "## Security exceptions this change adds\n- exception added: A\n- added: B";
        assert!(saved.ends_with(group), "{saved}");
        folder.write(report, r#"{"ok": true}"#);
        let saved = save(&folder.0, "c1", "{}").unwrap();
        assert_eq!(saved, render("c1", "{}", &NONE).unwrap());
        folder.write(report, "{not json");
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/g3.json: "), "{err}");
        fs::remove_file(folder.0.join(report)).unwrap();
        fs::create_dir_all(folder.0.join(report)).unwrap();
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        let start = "cannot read .gdd/changes/c1/g3.json: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn a_note_is_resolved_when_its_resolved_says_when_or_says_so() {
        for open in [Value::Null, Value::Bool(false), Value::from("")] {
            assert!(!is_resolved(&open), "{open}");
        }
        for done in [Value::from("x"), Value::Bool(true), Value::from(1)] {
            assert!(is_resolved(&done), "{done}");
        }
    }

    #[test]
    fn the_inbox_is_saved_next_to_the_record_as_it_is_rendered() {
        let folder = Folder::new();
        let saved = save(&folder.0, "c1", RECORD).unwrap();
        assert_eq!(saved, render("c1", RECORD, &NONE).unwrap());
        let text = fs::read_to_string(folder.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        assert_eq!(text, format!("{saved}\n"));
        let err = save(&folder.0, "c1", "{not json").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/change.json: "), "{err}");
        fs::remove_file(folder.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        let err = save(&folder.0, "c1", RECORD).unwrap_err();
        let start = "cannot write .gdd/changes/c1/inbox.md: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn a_record_that_is_not_valid_is_an_error() {
        for record in [
            "{not json",
            "3",
            r#"{"inbox": [{"text": 3}]}"#,
            r#"{"title": 1}"#,
        ] {
            assert!(render("c1", record, &NONE).is_err(), "{record}");
        }
    }
}
