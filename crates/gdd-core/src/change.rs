//! A change and its record, `.gdd/changes/<id>/change.json`: what it plans to touch, where its
//! build started and what its gates approved.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::classify::{Change, Classification};
use crate::config::Color;

/// Whether `id` can name a change: a lowercase letter or digit, then up to 40 lowercase
/// letters, digits or hyphens. Nothing else, so an id never leaves `.gdd/changes/`.
pub fn valid_id(id: &str) -> bool {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    id.len() <= 41 && id.starts_with(allowed) && id.chars().all(|c| allowed(c) || c == '-')
}

/// Where the record of change `id` lives, from the project root.
pub fn record_path(id: &str) -> String {
    format!(".gdd/changes/{id}/change.json")
}

/// Where the last snapshot of change `id` lives, from the project root.
pub fn snapshot_path(id: &str) -> String {
    format!(".gdd/changes/{id}/snapshot.json")
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
}

#[derive(Deserialize)]
struct RawRecord {
    #[serde(default)]
    paths: Vec<String>,
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
    /// paths, not resumable, no minimum color, not prepared, nothing decided or approved. An
    /// empty `build_start` is no build start. An unknown color is an error that names it.
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
        })
    }
}

/// What `gdd classify` prints: the id of the change, then its classification.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Decision<'a> {
    /// Id of the change.
    pub change: &'a str,
    /// Its classification.
    #[serde(flatten)]
    pub classification: &'a Classification,
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(branch("save"), "change/save");
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
                "decision": {"color": "red", "lane": "full"},
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
        let json = serde_json::to_string(&Decision {
            change: "save",
            classification: &classification,
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"change":"save","color":"red","lane":"full","reasons":["a.rs -> red (no zone: default color)"],"g1":false,"features":[],"tests_first":true,"mutation":true,"reviewer":true,"g4":true,"human_plan":true}"#
        );
    }
}
