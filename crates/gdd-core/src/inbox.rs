//! The Decision Inbox of a change: what the human must decide, what was decided without them
//! and what was parked, from the notes of its record.

use serde::Deserialize;
use serde_json::Value;

use crate::change::NOTE_KINDS;

/// The title of the group of each kind of note, in the order of [`NOTE_KINDS`].
const HEADINGS: [&str; 3] = ["Needs your decision", "Decided without you", "Parked"];

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

/// The Decision Inbox of change `id`, from the text of its record, without a line break at the
/// end: its title, color, lane and state, then one group per kind of note with the notes still
/// open (`Nothing.` when there are none) and, among the decisions, the paths outside the
/// package the scope check accepted; last, how many requests were resolved before the last
/// launch, if any. Notes of other kinds are left out.
pub fn render(id: &str, record: &str) -> Result<String, serde_json::Error> {
    let raw: Raw = serde_json::from_str(record)?;
    let title = raw.title.unwrap_or_default();
    let decision = raw.decision.unwrap_or_default();
    let color = decision.color.unwrap_or_else(|| "?".into());
    let lane = decision.lane.unwrap_or_else(|| "?".into());
    let state = raw.state.unwrap_or_else(|| "?".into());
    let mut groups: [Vec<String>; 3] = Default::default();
    let mut resolved = 0;
    for note in raw.inbox.unwrap_or_default() {
        let group = NOTE_KINDS.iter().position(|kind| *kind == note.kind);
        if is_resolved(&note.resolved) {
            resolved += 1;
        } else if let Some(group) = group {
            groups[group].push(note.text);
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
    if resolved > 0 {
        blocks.push(format!("_{resolved} {RESOLVED}_"));
    }
    Ok(blocks.join("\n\n"))
}

/// Whether a note was resolved: its `resolved` says when, or says so.
fn is_resolved(resolved: &Value) -> bool {
    match resolved {
        Value::Null | Value::Bool(false) => false,
        Value::String(when) => !when.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(render("c1", RECORD).unwrap(), expected);
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
        assert_eq!(render("c2", "{}").unwrap(), expected);
        let nulls = r#"{"decision": null, "inbox": null, "out_of_scope": null}"#;
        assert_eq!(render("c2", nulls).unwrap(), expected);
        let empty = r#"{"decision": {}, "inbox": [], "out_of_scope": []}"#;
        assert_eq!(render("c2", empty).unwrap(), expected);
    }

    #[test]
    fn parked_notes_have_their_group() {
        let record = r#"{"inbox": [{"kind": "parked", "text": "Waiting for the API"}]}"#;
        let text = render("c1", record).unwrap();
        let end = "## Parked\n- Waiting for the API";
        assert!(text.ends_with(end), "{text}");
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
    fn a_record_that_is_not_valid_is_an_error() {
        for record in [
            "{not json",
            "3",
            r#"{"inbox": [{"text": 3}]}"#,
            r#"{"title": 1}"#,
        ] {
            assert!(render("c1", record).is_err(), "{record}");
        }
    }
}
