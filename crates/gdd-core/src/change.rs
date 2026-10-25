//! A planned change: what its record, `.gdd/changes/<id>/change.json`, says it will touch.

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

/// The fields of a change record that classifying needs; the others are ignored.
#[derive(Deserialize)]
struct Record {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    resumable: bool,
    #[serde(default, deserialize_with = "color")]
    min_color: Option<Color>,
}

fn color<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Color>, D::Error> {
    match Option::<String>::deserialize(deserializer)? {
        Some(name) => name.parse().map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// Reads a change record (JSON): its planned paths, whether it is resumable and its declared
/// minimum color (`null` or missing: none). An unknown color is an error that names it.
pub fn from_json(text: &str) -> Result<Change, serde_json::Error> {
    let record: Record = serde_json::from_str(text)?;
    Ok(Change {
        paths: record.paths,
        min_color: record.min_color,
        resumable: record.resumable,
    })
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
    fn the_record_lives_in_the_folder_of_the_change() {
        assert_eq!(record_path("save"), ".gdd/changes/save/change.json");
    }

    #[test]
    fn a_record_gives_the_paths_the_minimum_and_resumable() {
        let change = from_json(
            r#"{"id": "x", "title": "X", "paths": ["a.rs", "b.rs"], "resumable": true,
                "min_color": "red", "state": "open", "gates": {}, "inbox": []}"#,
        )
        .unwrap();
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
    fn missing_fields_and_a_null_minimum_take_their_defaults() {
        let empty = Change {
            paths: vec![],
            min_color: None,
            resumable: false,
        };
        assert_eq!(from_json("{}").unwrap(), empty);
        assert_eq!(from_json(r#"{"min_color": null}"#).unwrap(), empty);
    }

    #[test]
    fn an_unknown_minimum_color_is_an_error_that_names_it() {
        let err = from_json(r#"{"min_color": "purple"}"#).unwrap_err();
        assert!(err.to_string().contains("purple"), "{err}");
    }

    #[test]
    fn a_record_that_is_not_valid_is_an_error() {
        assert!(from_json("").is_err());
        assert!(from_json("{not json").is_err());
        assert!(from_json(r#"{"paths": "a.rs"}"#).is_err());
    }

    #[test]
    fn the_decision_starts_with_the_change_and_follows_the_classification() {
        let zones = ZoneMap::from_toml("default = \"red\"").unwrap();
        let change = from_json(r#"{"paths": ["a.rs"]}"#).unwrap();
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
