//! A change and its record, `.gdd/changes/<id>/change.json`: what it plans to touch, where its
//! build started, what its gates decided, the notes left on it and its state. Parking a change
//! stops it with a note for the human.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::agents::models::Roles;
use crate::classify::{Change, Classification};
use crate::config::Color;
use crate::{inbox, log};

/// The kind of work of a change whose record says none.
const DEFAULT_KIND: &str = "Feature";

/// The kinds of note a change keeps for its Decision Inbox: a decision the human must take, a
/// decision taken without them, and work parked.
pub const NOTE_KINDS: [&str; 3] = ["needs", "decided", "parked"];

/// Whether `id` can name a change: a lowercase letter or digit, then up to 40 lowercase
/// letters, digits or hyphens. Nothing else, so an id never leaves `.gdd/changes/`.
pub fn valid_id(id: &str) -> bool {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    id.len() <= 41 && id.starts_with(allowed) && id.chars().all(|c| allowed(c) || c == '-')
}

/// `Ok` when `id` can name a change (see [`valid_id`]); else why not.
pub fn check_id(id: &str) -> Result<(), String> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(format!(
            "invalid change id {id:?}: use lowercase letters, digits and hyphens"
        ))
    }
}

/// Where the record of change `id` lives, from the project root.
pub fn record_path(id: &str) -> String {
    format!(".gdd/changes/{id}/change.json")
}

/// Where the Decision Inbox of change `id` lives, from the project root.
pub fn inbox_path(id: &str) -> String {
    format!(".gdd/changes/{id}/inbox.md")
}

/// The text of the record of change `id` of the project at `root`. An id that cannot name a
/// change, a change without a record and a record that cannot be read are errors that say so.
pub fn read_record(root: &Path, id: &str) -> Result<String, String> {
    check_id(id)?;
    let path = record_path(id);
    match fs::read_to_string(root.join(&path)) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            Err(format!("change {id} does not exist: there is no {path}"))
        }
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// What a new change starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct New<'a> {
    /// Its id.
    pub id: &'a str,
    /// What it is about, in a few words.
    pub title: &'a str,
    /// The kind of work: `Feature`, `Hotfix`...
    pub kind: &'a str,
    /// The paths it plans to touch.
    pub paths: &'a [String],
    /// Whether it can be resumed after an interruption.
    pub resumable: bool,
    /// Its declared minimum color.
    pub min_color: Option<Color>,
    /// When it was created (see [`crate::log::now`]).
    pub created: &'a str,
}

#[derive(Serialize)]
struct Fresh<'a> {
    id: &'a str,
    title: &'a str,
    #[serde(rename = "type")]
    kind: &'a str,
    paths: &'a [String],
    resumable: bool,
    min_color: Option<String>,
    created: &'a str,
    state: &'a str,
    gates: Map<String, Value>,
    inbox: Vec<Value>,
}

/// The record of a new change: open, without gates or notes.
pub fn new_record(new: &New) -> Result<String, serde_json::Error> {
    written(&Fresh {
        id: new.id,
        title: new.title,
        kind: new.kind,
        paths: new.paths,
        resumable: new.resumable,
        min_color: new.min_color.map(|color| color.to_string()),
        created: new.created,
        state: "open",
        gates: Map::new(),
        inbox: Vec::new(),
    })
}

/// A gate decision as a record keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Approval<'a> {
    /// `ok` or `ko`.
    pub decision: &'a str,
    /// Who decided: `human`, `machines`...
    pub by: &'a str,
    /// When (see [`crate::log::now`]).
    pub ts: &'a str,
    /// What to read about it.
    pub note: &'a str,
    /// The scenario files G1 sealed, with their SHA-256.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashes: Option<&'a BTreeMap<String, String>>,
}

/// The record `text` with `approval` as the decision on `gate` and, when given, `features` as
/// the scenario files of the change; every other field keeps its value.
pub fn with_gate(
    text: &str,
    gate: &str,
    approval: &Approval,
    features: Option<&[String]>,
) -> Result<String, serde_json::Error> {
    let mut record: Map<String, Value> = serde_json::from_str(text)?;
    let approval = serde_json::to_value(approval)?;
    let gates = field(&mut record, "gates", Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| wrong("gates", "an object"))?;
    gates.insert(gate.to_string(), approval);
    if let Some(features) = features {
        record.insert("features".to_string(), Value::from(features));
    }
    written(&record)
}

/// A note for the Decision Inbox of a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Note<'a> {
    /// One of [`NOTE_KINDS`].
    pub kind: &'a str,
    /// What it says.
    pub text: &'a str,
    /// When it was left (see [`crate::log::now`]).
    pub ts: &'a str,
}

/// The record `text` with `note` at the end of its notes, `inbox`; every other field keeps its
/// value.
pub fn with_note(text: &str, note: &Note) -> Result<String, serde_json::Error> {
    let mut record: Map<String, Value> = serde_json::from_str(text)?;
    let note = serde_json::to_value(note)?;
    field(&mut record, "inbox", Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| wrong("inbox", "a list"))?
        .push(note);
    written(&record)
}

/// The record `text` with each of `fields` set to its value; every other field keeps its value.
pub fn with_fields(text: &str, fields: Vec<(&str, Value)>) -> Result<String, serde_json::Error> {
    let mut record: Map<String, Value> = serde_json::from_str(text)?;
    for (key, value) in fields {
        record.insert(key.to_string(), value);
    }
    written(&record)
}

/// The record `text` with its requests for the human still open (`needs` notes) resolved at
/// `ts`: a (re)launch means the Orchestrator dealt with them. They stay in the record, out of
/// the Decision Inbox. Every other field keeps its value.
pub fn with_requests_resolved(text: &str, ts: &str) -> Result<String, serde_json::Error> {
    let mut record: Map<String, Value> = serde_json::from_str(text)?;
    if let Some(Value::Array(notes)) = record.get_mut("inbox") {
        for note in notes.iter_mut().filter_map(Value::as_object_mut) {
            let request = note.get("kind").and_then(Value::as_str) == Some("needs");
            let resolved = note.get("resolved").is_some_and(inbox::is_resolved);
            if request && !resolved {
                note.insert("resolved".to_string(), Value::from(ts));
            }
        }
    }
    written(&record)
}

/// The kind of work the record `text` says its change is (`Feature`, `Spike`...): `Feature`
/// when it says none.
pub fn kind(text: &str) -> Result<String, serde_json::Error> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default, rename = "type")]
        kind: Option<String>,
    }
    let raw: Raw = serde_json::from_str(text)?;
    Ok(raw.kind.unwrap_or_else(|| DEFAULT_KIND.to_string()))
}

/// Parks change `id` of the project at `root` for `reason`: a `parked` note in its record, which
/// goes to state `parked`; logged, and its Decision Inbox saved (see [`inbox::save`]). What to
/// tell the human, in one line.
pub fn park(root: &Path, id: &str, reason: &str) -> Result<String, String> {
    let text = read_record(root, id)?;
    let path = record_path(id);
    let ts = log::now();
    let note = Note {
        kind: "parked",
        text: reason,
        ts: &ts,
    };
    let parked = with_note(&text, &note)
        .and_then(|text| with_fields(&text, vec![("state", Value::from("parked"))]))
        .map_err(|e| format!("{path}: {e}"))?;
    write(root, &path, &parked)?;
    let entry = log::Entry {
        change: id.to_string(),
        phase: "-".to_string(),
        gate: "-".to_string(),
        role: "machine".to_string(),
        agent: "-".to_string(),
        result: "parked".to_string(),
        duration_s: 0.0,
        note: reason.to_string(),
    };
    logged(root, &entry)?;
    inbox::save(root, id, &parked)?;
    let inbox = inbox_path(id);
    Ok(format!("Change {id} parked: {reason}. Message in {inbox}"))
}

/// Writes `text` to the file `path` of the project at `root`, creating its folder.
pub fn write(root: &Path, path: &str, text: &str) -> Result<(), String> {
    let file = root.join(path);
    file.parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&file, text))
        .map_err(|e| format!("cannot write {path}: {e}"))
}

/// Adds `entry`, done now, to the log of the project at `root`.
pub fn logged(root: &Path, entry: &log::Entry) -> Result<(), String> {
    let line = entry.line(&log::now());
    let line = line.map_err(|e| format!("{}: {e}", log::PATH))?;
    log::append(root, &line).map_err(|e| format!("cannot write {}: {e}", log::PATH))
}

/// The field `key` of `record`; `empty` when the record has none, or `null`.
fn field<'a>(record: &'a mut Map<String, Value>, key: &str, empty: Value) -> &'a mut Value {
    let value = record.entry(key).or_insert(Value::Null);
    if value.is_null() {
        *value = empty;
    }
    value
}

/// The error of a field of a record that holds something else than `what`.
fn wrong(key: &str, what: &str) -> serde_json::Error {
    serde_json::Error::custom(format!("the field {key:?} of the record is not {what}"))
}

/// `record` as the tool writes records: indented JSON, with a line break at the end.
fn written(record: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(record).map(|text| format!("{text}\n"))
}

/// Where the last snapshot of change `id` lives, from the project root.
pub fn snapshot_path(id: &str) -> String {
    format!(".gdd/changes/{id}/snapshot.json")
}

/// Where the snapshot taken after the test mode of change `id` lives, from the project root:
/// build mode is judged from it.
pub fn build_snapshot_path(id: &str) -> String {
    format!(".gdd/changes/{id}/snapshot-build.json")
}

/// The record `text` with `accepted`, the paths outside the plan the scope check accepted, as
/// its `out_of_scope`; `None` when there is nothing to write: nothing accepted, or the record
/// says it already. The other fields keep their values.
pub fn with_out_of_scope(
    text: &str,
    accepted: &[String],
) -> Result<Option<String>, serde_json::Error> {
    let mut record: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text)?;
    let paths = serde_json::Value::from(accepted);
    if accepted.is_empty() || record.get("out_of_scope") == Some(&paths) {
        return Ok(None);
    }
    record.insert("out_of_scope".to_string(), paths);
    serde_json::to_string_pretty(&record).map(|text| Some(format!("{text}\n")))
}

/// The git branch of change `id`.
pub fn branch(id: &str) -> String {
    format!("change/{id}")
}

/// The fields of a change record the tool reads; the others are ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// What the change plans: its paths, its declared minimum color and whether it is resumable.
    pub plan: Change,
    /// Commit where its build started; `None` until the change is prepared.
    pub build_start: Option<String>,
    /// Color decided when the change was prepared; `None` until then.
    pub decided_color: Option<Color>,
    /// Scenario files approved in G1, with the SHA-256 they had then.
    pub approved: BTreeMap<String, String>,
    /// Scenario files of the change, as G1 recorded them: hidden ones too, which never show in
    /// what it plans or touches.
    pub features: Vec<String>,
}

#[derive(Deserialize)]
struct RawRecord {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    resumable: bool,
    #[serde(default, deserialize_with = "color")]
    min_color: Option<Color>,
    #[serde(default)]
    build_start: Option<String>,
    #[serde(default)]
    decision: Option<RawDecision>,
    #[serde(default)]
    gates: RawGates,
}

#[derive(Deserialize)]
struct RawDecision {
    #[serde(default, deserialize_with = "color")]
    color: Option<Color>,
}

#[derive(Default, Deserialize)]
struct RawGates {
    #[serde(rename = "G1")]
    g1: Option<RawApproval>,
}

#[derive(Deserialize)]
struct RawApproval {
    #[serde(default)]
    hashes: BTreeMap<String, String>,
}

fn color<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Color>, D::Error> {
    match Option::<String>::deserialize(deserializer)? {
        Some(name) => name.parse().map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

impl Record {
    /// Reads a change record (JSON). A field it leaves out, or `null`, takes its default: no
    /// paths, not resumable, no minimum color, not prepared, nothing decided or approved, no
    /// scenarios. An empty `build_start` is no build start. An unknown color is an error that
    /// names it.
    pub fn from_json(text: &str) -> Result<Record, serde_json::Error> {
        let raw: RawRecord = serde_json::from_str(text)?;
        Ok(Record {
            plan: Change {
                paths: raw.paths,
                min_color: raw.min_color,
                resumable: raw.resumable,
            },
            build_start: raw.build_start.filter(|start| !start.is_empty()),
            decided_color: raw.decision.and_then(|d| d.color),
            approved: raw.gates.g1.map(|g1| g1.hashes).unwrap_or_default(),
            features: raw.features,
        })
    }
}

/// What `gdd classify` prints: the id of the change, its classification, its kind of work, the
/// model of each role and the Builder rounds against G2.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Decision<'a> {
    /// Id of the change.
    pub change: &'a str,
    /// Its classification.
    #[serde(flatten)]
    pub classification: &'a Classification,
    /// The kind of work: `Feature`, `Spike`...
    #[serde(rename = "type")]
    pub kind: &'a str,
    /// The model of each role.
    #[serde(flatten)]
    pub roles: &'a Roles,
    /// Builder rounds against G2 before the change is parked.
    pub retries: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::models::Models;
    use crate::git::tests::Folder;
    use crate::{Lane, classify, rules, zones::ZoneMap};

    fn plan(text: &str) -> Change {
        Record::from_json(text).unwrap().plan
    }

    #[test]
    fn ids_are_lowercase_letters_digits_and_hyphens() {
        for id in ["a", "0", "core-classify", "x-1-"] {
            assert!(valid_id(id), "{id:?} is valid");
        }
        for id in ["", "-a", "A", "aB", "a_b", "a/b", "..", "../x", "a b", "é"] {
            assert!(!valid_id(id), "{id:?} is not valid");
        }
        assert!(valid_id(&"a".repeat(41)));
        assert!(!valid_id(&"a".repeat(42)));
    }

    #[test]
    fn the_files_of_a_change_live_in_its_folder_and_its_work_on_its_branch() {
        assert_eq!(record_path("save"), ".gdd/changes/save/change.json");
        assert_eq!(snapshot_path("save"), ".gdd/changes/save/snapshot.json");
        let build = ".gdd/changes/save/snapshot-build.json";
        assert_eq!(build_snapshot_path("save"), build);
        assert_eq!(branch("save"), "change/save");
    }

    #[test]
    fn an_id_that_cannot_name_a_change_says_why() {
        assert_eq!(check_id("c1"), Ok(()));
        let why = "invalid change id \"../c1\": use lowercase letters, digits and hyphens";
        assert_eq!(check_id("../c1"), Err(why.to_string()));
        assert_eq!(inbox_path("c1"), ".gdd/changes/c1/inbox.md");
    }

    #[test]
    fn a_record_is_read_only_for_a_change_that_exists() {
        let folder = Folder::new();
        let text = "{\"id\": \"c1\"}";
        folder.write(".gdd/changes/c1/change.json", text);
        assert_eq!(read_record(&folder.0, "c1"), Ok(text.to_string()));
        let invalid = check_id("C1").unwrap_err();
        assert_eq!(read_record(&folder.0, "C1"), Err(invalid));
        let missing = "change c2 does not exist: there is no .gdd/changes/c2/change.json";
        assert_eq!(read_record(&folder.0, "c2"), Err(missing.into()));
        fs::create_dir_all(folder.0.join(".gdd/changes/c3/change.json")).unwrap();
        let unreadable = read_record(&folder.0, "c3").unwrap_err();
        let start = "cannot read .gdd/changes/c3/change.json: ";
        assert!(unreadable.starts_with(start), "{unreadable}");
    }

    #[test]
    fn a_new_change_is_open_without_gates_or_notes() {
        let paths = ["src/a.rs".to_string(), "docs/é.md".to_string()];
        let new = New {
            id: "c1",
            title: "Drafts",
            kind: "Feature",
            paths: &paths,
            resumable: true,
            min_color: Some(Color::Red),
            created: "2026-10-26T07:12:29+00:00",
        };
        let expected = "{\n  \"id\": \"c1\",\n  \"title\": \"Drafts\",\n  \
                        \"type\": \"Feature\",\n  \
                        \"paths\": [\n    \"src/a.rs\",\n    \"docs/é.md\"\n  ],\n  \
                        \"resumable\": true,\n  \"min_color\": \"red\",\n  \
                        \"created\": \"2026-10-26T07:12:29+00:00\",\n  \
                        \"state\": \"open\",\n  \"gates\": {},\n  \"inbox\": []\n}\n";
        assert_eq!(new_record(&new).unwrap(), expected);
        let plain = New {
            paths: &[],
            resumable: false,
            min_color: None,
            ..new
        };
        let record = Record::from_json(&new_record(&plain).unwrap()).unwrap();
        assert_eq!(record.plan.paths, Vec::<String>::new());
        assert_eq!(record.plan.min_color, None);
        assert!(!record.plan.resumable);
    }

    fn approval<'a>(hashes: Option<&'a BTreeMap<String, String>>) -> Approval<'a> {
        Approval {
            decision: "ok",
            by: "human",
            ts: "2026-10-26T07:21:55+00:00",
            note: "Looks fine",
            hashes,
        }
    }

    #[test]
    fn a_gate_decision_is_written_in_the_record_and_keeps_the_rest() {
        let text = r#"{"id": "c1", "gates": {"plan": {"decision": "ok"}}, "x": [1]}"#;
        let written = with_gate(text, "G4", &approval(None), None).unwrap();
        assert!(written.ends_with("}\n"));
        assert!(written.contains("\n  \"id\": \"c1\""));
        let value: Value = serde_json::from_str(&written).unwrap();
        let expected = serde_json::json!({
            "id": "c1",
            "gates": {
                "plan": {"decision": "ok"},
                "G4": {
                    "decision": "ok",
                    "by": "human",
                    "ts": "2026-10-26T07:21:55+00:00",
                    "note": "Looks fine",
                },
            },
            "x": [1],
        });
        assert_eq!(value, expected);
        let file = "features/a.feature".to_string();
        let hashes = BTreeMap::from([(file.clone(), "ab12".to_string())]);
        let features = [file];
        let sealed = with_gate(text, "G1", &approval(Some(&hashes)), Some(&features)).unwrap();
        let record = Record::from_json(&sealed).unwrap();
        assert_eq!(record.approved, hashes);
        assert_eq!(record.features, features);
        let value: Value = serde_json::from_str(&sealed).unwrap();
        let plan = serde_json::json!({"decision": "ok"});
        assert_eq!(value["gates"]["plan"], plan);
    }

    #[test]
    fn a_record_without_gates_gets_them_and_one_that_is_not_an_object_is_an_error() {
        let plan = |text| with_gate(text, "plan", &approval(None), None);
        for text in ["{}", r#"{"gates": null}"#] {
            let value: Value = serde_json::from_str(&plan(text).unwrap()).unwrap();
            assert_eq!(value["gates"]["plan"]["by"], "human", "{text}");
            assert_eq!(value.get("features"), None);
        }
        let err = plan(r#"{"gates": []}"#).unwrap_err();
        assert_eq!(
            err.to_string(),
            "the field \"gates\" of the record is not an object"
        );
        assert!(plan("[]").is_err());
        assert!(plan("{not json").is_err());
    }

    #[test]
    fn notes_are_added_at_the_end_of_the_inbox() {
        let note = Note {
            kind: "needs",
            text: "Pick a name",
            ts: "2026-10-26T08:00:00+00:00",
        };
        let text = r#"{"id": "c1", "inbox": [{"kind": "decided", "text": "Old"}]}"#;
        let written = with_note(text, &note).unwrap();
        assert!(written.ends_with("}\n"));
        let value: Value = serde_json::from_str(&written).unwrap();
        let expected = serde_json::json!({
            "id": "c1",
            "inbox": [
                {"kind": "decided", "text": "Old"},
                {"kind": "needs", "text": "Pick a name", "ts": "2026-10-26T08:00:00+00:00"},
            ],
        });
        assert_eq!(value, expected);
        for text in ["{}", r#"{"inbox": null}"#] {
            let written = with_note(text, &note).unwrap();
            let value: Value = serde_json::from_str(&written).unwrap();
            assert_eq!(value["inbox"][0]["text"], "Pick a name", "{text}");
        }
        let err = with_note(r#"{"inbox": {}}"#, &note).unwrap_err();
        assert_eq!(
            err.to_string(),
            "the field \"inbox\" of the record is not a list"
        );
        assert!(with_note("\"x\"", &note).is_err());
        assert_eq!(NOTE_KINDS, ["needs", "decided", "parked"]);
    }

    #[test]
    fn the_paths_the_scope_accepted_are_written_in_the_record() {
        let accepted = ["docs/a.md".to_string(), "src/é.rs".to_string()];
        let text = r#"{"id": "x", "paths": ["a.rs"], "gates": {"G1": {"decision": "ok"}}}"#;
        let written = with_out_of_scope(text, &accepted).unwrap().unwrap();
        assert!(written.ends_with("}\n"), "{written}");
        assert!(written.contains("\n  \"id\": \"x\",\n"), "{written}");
        assert!(written.contains("\"src/é.rs\""), "{written}");
        let value: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "id": "x",
                "paths": ["a.rs"],
                "gates": {"G1": {"decision": "ok"}},
                "out_of_scope": ["docs/a.md", "src/é.rs"],
            })
        );
        assert_eq!(Record::from_json(&written).unwrap().plan.paths, ["a.rs"]);
        assert_eq!(with_out_of_scope(&written, &accepted).unwrap(), None);
        let first = &accepted[..1];
        let changed = with_out_of_scope(&written, first).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_str(&changed).unwrap();
        assert_eq!(value["out_of_scope"], serde_json::json!(["docs/a.md"]));
        assert_eq!(with_out_of_scope(&written, &[]).unwrap(), None);
        assert_eq!(with_out_of_scope("{}", &[]).unwrap(), None);
        assert!(with_out_of_scope("[]", &accepted).is_err());
        assert!(with_out_of_scope("{not json", &accepted).is_err());
    }

    #[test]
    fn a_record_gives_the_paths_the_minimum_and_resumable() {
        let change = plan(
            r#"{"id": "x", "title": "X", "paths": ["a.rs", "b.rs"], "resumable": true,
                "min_color": "red", "state": "open", "gates": {}, "inbox": []}"#,
        );
        assert_eq!(
            change,
            Change {
                paths: vec!["a.rs".into(), "b.rs".into()],
                min_color: Some(Color::Red),
                resumable: true,
            }
        );
    }

    #[test]
    fn a_prepared_record_gives_its_start_its_color_and_its_approved_scenarios() {
        let record = Record::from_json(
            r#"{"paths": ["a.rs"], "build_start": "37cc864",
                "decision": {"color": "red", "lane": "full", "features": ["x.feature"]},
                "features": ["features/a.feature", "features/hidden/a.feature"],
                "gates": {"G1": {"decision": "ok", "hashes": {"features/a.feature": "ab12"}},
                          "plan": {"decision": "ok"}}}"#,
        )
        .unwrap();
        assert_eq!(record.build_start.as_deref(), Some("37cc864"));
        assert_eq!(record.decided_color, Some(Color::Red));
        assert_eq!(
            record.approved,
            BTreeMap::from([("features/a.feature".to_string(), "ab12".to_string())])
        );
        let features = ["features/a.feature", "features/hidden/a.feature"];
        assert_eq!(record.features, features);
    }

    #[test]
    fn missing_fields_and_nulls_take_their_defaults() {
        let empty = Record {
            plan: Change {
                paths: vec![],
                min_color: None,
                resumable: false,
            },
            build_start: None,
            decided_color: None,
            approved: BTreeMap::new(),
            features: vec![],
        };
        assert_eq!(Record::from_json("{}").unwrap(), empty);
        for text in [
            r#"{"min_color": null, "build_start": null, "decision": null}"#,
            r#"{"build_start": "", "decision": {}, "gates": {"G1": {}}}"#,
            r#"{"decision": {"color": null}, "gates": {"G1": null, "G2": {"hashes": {"a": "b"}}}}"#,
        ] {
            assert_eq!(Record::from_json(text).unwrap(), empty, "{text}");
        }
    }

    #[test]
    fn an_unknown_color_is_an_error_that_names_it() {
        let err = Record::from_json(r#"{"min_color": "purple"}"#).unwrap_err();
        assert!(err.to_string().contains("purple"), "{err}");
        let err = Record::from_json(r#"{"decision": {"color": "blue"}}"#).unwrap_err();
        assert!(err.to_string().contains("blue"), "{err}");
    }

    #[test]
    fn a_record_that_is_not_valid_is_an_error() {
        for text in [
            "",
            "{not json",
            r#"{"paths": "a.rs"}"#,
            r#"{"build_start": 7}"#,
            r#"{"gates": {"G1": {"hashes": ["a"]}}}"#,
            r#"{"features": "features/a.feature"}"#,
        ] {
            assert!(Record::from_json(text).is_err(), "{text}");
        }
    }

    #[test]
    fn the_decision_starts_with_the_change_and_follows_the_classification() {
        let zones = ZoneMap::from_toml("default = \"red\"").unwrap();
        let change = plan(r#"{"paths": ["a.rs"]}"#);
        let classification = classify(&zones, &rules::default_table(), &change).unwrap();
        assert_eq!(classification.lane, Lane::Full);
        let models = "[builder.red]\nintegration = \"claude\"\nmodel = \"opus\"\n";
        let roles = Models::from_toml(models).unwrap().roles(Color::Red);
        let json = serde_json::to_string(&Decision {
            change: "save",
            classification: &classification,
            kind: "Spike",
            roles: &roles,
            retries: 3,
        })
        .unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"change":"save","color":"red","lane":"full","#,
                r#""reasons":["a.rs -> red (no zone: default color)"],"#,
                r#""g1":false,"features":[],"tests_first":true,"mutation":true,"#,
                r#""reviewer":true,"g4":true,"human_plan":true,"#,
                r#""type":"Spike","builder":{"integration":"claude","model":"opus"},"#,
                r#""builder_test":{},"reviewer_model":{},"orchestrator":{},"retries":3}"#
            )
        );
    }

    #[test]
    fn fields_are_set_and_the_rest_keep_their_values() {
        let text = r#"{"id": "c1", "state": "open", "x": [1]}"#;
        let fields = vec![
            ("state", Value::from("merged")),
            ("commit", Value::from("ab")),
        ];
        let written = with_fields(text, fields).unwrap();
        assert!(written.ends_with("}\n"), "{written}");
        assert!(written.contains("\n  \"id\": \"c1\""), "{written}");
        let value: Value = serde_json::from_str(&written).unwrap();
        let expected = r#"{"id": "c1", "state": "merged", "x": [1], "commit": "ab"}"#;
        assert_eq!(value, serde_json::from_str::<Value>(expected).unwrap());
        let same: Value = serde_json::from_str(&with_fields(text, vec![]).unwrap()).unwrap();
        assert_eq!(same, serde_json::from_str::<Value>(text).unwrap());
        assert!(with_fields("[]", vec![]).is_err());
        assert!(with_fields("{not json", vec![]).is_err());
    }

    #[test]
    fn a_launch_resolves_the_requests_still_open_and_keeps_them() {
        let text = r#"{"id": "c1", "inbox": [
            {"kind": "needs", "text": "Open"},
            {"kind": "needs", "text": "Null", "resolved": null},
            {"kind": "needs", "text": "Empty", "resolved": ""},
            {"kind": "needs", "text": "Done", "resolved": "2026-01-02"},
            {"kind": "decided", "text": "Kept"},
            {"kind": "parked", "text": "Waiting"},
            "not a note"
        ]}"#;
        let ts = "2026-10-26T10:00:00+00:00";
        let written = with_requests_resolved(text, ts).unwrap();
        assert!(written.ends_with("}\n"), "{written}");
        let value: Value = serde_json::from_str(&written).unwrap();
        let notes = value["inbox"].as_array().unwrap();
        let resolved: Vec<&Value> = notes.iter().map(|note| &note["resolved"]).collect();
        let (ts, done, null) = (Value::from(ts), Value::from("2026-01-02"), Value::Null);
        let expected = [&ts, &ts, &ts, &done, &null, &null, &null];
        assert_eq!(resolved, expected);
        assert_eq!(notes[0]["text"], "Open");
        assert_eq!(notes[6], "not a note");
        assert_eq!(value["id"], "c1");
        for text in ["{}", r#"{"inbox": null}"#, r#"{"inbox": {}}"#] {
            let written = with_requests_resolved(text, "x").unwrap();
            let value: Value = serde_json::from_str(&written).unwrap();
            let unchanged: Value = serde_json::from_str(text).unwrap();
            assert_eq!(value, unchanged, "{text}");
        }
        assert!(with_requests_resolved("[]", "x").is_err());
    }

    #[test]
    fn the_kind_of_work_is_the_type_of_the_record_or_feature() {
        assert_eq!(kind(r#"{"type": "Spike"}"#).unwrap(), "Spike");
        for text in ["{}", r#"{"type": null, "title": "x"}"#] {
            assert_eq!(kind(text).unwrap(), "Feature", "{text}");
        }
        for wrong in [r#"{"type": 3}"#, "3", "{not json"] {
            assert!(kind(wrong).is_err(), "{wrong}");
        }
    }

    const PARKED: &str = "G2 still red after 3 retries";

    fn read_json(folder: &Folder, path: &str) -> Value {
        serde_json::from_str(&fs::read_to_string(folder.0.join(path)).unwrap()).unwrap()
    }

    #[test]
    fn parking_stops_the_change_with_a_note_logs_it_and_saves_its_inbox() {
        let folder = Folder::new();
        let record = r#"{"id": "c1", "title": "App", "state": "building", "x": 1,
                         "inbox": [{"kind": "needs", "text": "Which format"}]}"#;
        folder.write(".gdd/changes/c1/change.json", record);
        let said = park(&folder.0, "c1", PARKED).unwrap();
        let saved = ".gdd/changes/c1/inbox.md";
        let expected = format!("Change c1 parked: {PARKED}. Message in {saved}");
        assert_eq!(said, expected);
        let text = fs::read_to_string(folder.0.join(".gdd/changes/c1/change.json")).unwrap();
        let record: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(record["state"], "parked");
        assert_eq!(record["x"], 1);
        assert_eq!(record["inbox"][0]["text"], "Which format");
        let note = &record["inbox"][1];
        assert_eq!(note["kind"], "parked");
        assert_eq!(note["text"], PARKED);
        assert!(note["ts"].as_str().unwrap().ends_with("+00:00"), "{note}");
        let inbox = fs::read_to_string(folder.0.join(saved)).unwrap();
        assert_eq!(inbox, format!("{}\n", inbox::render("c1", &text).unwrap()));
        let parked = format!("## Parked\n- {PARKED}");
        assert!(inbox.contains(&parked), "{inbox}");
        assert!(inbox.contains("state parked"), "{inbox}");
        let log = log::parse(&fs::read_to_string(folder.0.join(log::PATH)).unwrap()).unwrap();
        let [line] = log.as_slice() else {
            panic!("one log line: {log:?}");
        };
        for (field, value) in [
            ("change", "c1"),
            ("phase", "-"),
            ("gate", "-"),
            ("role", "machine"),
            ("agent", "-"),
            ("result", "parked"),
            ("note", PARKED),
        ] {
            assert_eq!(line.text(field), value, "{field}");
        }
    }

    #[test]
    fn parking_needs_a_record_it_can_read_and_files_it_can_write() {
        let folder = Folder::new();
        let missing = "change c9 does not exist: there is no .gdd/changes/c9/change.json";
        assert_eq!(park(&folder.0, "c9", PARKED), Err(missing.into()));
        let invalid = check_id("C9").unwrap_err();
        assert_eq!(park(&folder.0, "C9", PARKED), Err(invalid));
        folder.write(".gdd/changes/c1/change.json", r#"{"inbox": {}}"#);
        let err = park(&folder.0, "c1", PARKED).unwrap_err();
        let start = ".gdd/changes/c1/change.json: the field \"inbox\"";
        assert!(err.starts_with(start), "{err}");
        assert!(!folder.0.join(".gdd/changes/c1/inbox.md").exists());
        folder.write(".gdd/changes/c2/change.json", "{}");
        fs::create_dir_all(folder.0.join(log::PATH)).unwrap();
        let err = park(&folder.0, "c2", PARKED).unwrap_err();
        assert!(err.starts_with("cannot write .gdd/log.jsonl: "), "{err}");
        let record = read_json(&folder, ".gdd/changes/c2/change.json");
        assert_eq!(record["state"], "parked");
        fs::create_dir_all(folder.0.join(".gdd/changes/c3/change.json")).unwrap();
        let err = write(&folder.0, ".gdd/changes/c3/change.json", "{}").unwrap_err();
        let start = "cannot write .gdd/changes/c3/change.json: ";
        assert!(err.starts_with(start), "{err}");
    }
}
