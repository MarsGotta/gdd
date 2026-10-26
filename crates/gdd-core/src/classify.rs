//! Classifying a change: its color, the lane it travels and the gates it must pass.

use std::fmt;

use serde::{Serialize, Serializer};

use crate::config::{Color, Error, Lane};
use crate::rules::RulesTable;
use crate::zones::ZoneMap;

/// A change to classify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Paths the change touches.
    pub paths: Vec<String>,
    /// Declared minimum color: it raises the color, never lowers it.
    pub min_color: Option<Color>,
    /// Whether the change can be resumed after an interruption (a condition of the fast lane).
    pub resumable: bool,
}

/// The decision for a change: its color, lane and the gates it must pass.
/// As JSON, the color and the lane are their names (`"red"`, `"full"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Classification {
    /// Risk color.
    #[serde(serialize_with = "as_text")]
    pub color: Color,
    /// Lane it travels.
    #[serde(serialize_with = "as_text")]
    pub lane: Lane,
    /// Why: one line per path, plus the declared minimum when it raised the color.
    pub reasons: Vec<String>,
    /// New or changed scenarios need approval (G1).
    pub g1: bool,
    /// Scenario files the change touches (regression scenarios not included).
    pub features: Vec<String>,
    /// Tests are written before the code.
    pub tests_first: bool,
    /// Mutation testing runs.
    pub mutation: bool,
    /// The Reviewer checks the change.
    pub reviewer: bool,
    /// The human release gate (G4) applies.
    pub g4: bool,
    /// The human approves the plan.
    pub human_plan: bool,
}

fn as_text<T: fmt::Display, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// Classifies a change: the highest color among its paths (or the default color when it has
/// none), raised to its declared minimum, then the lane and gates the rules table gives it.
/// Paths are taken sorted and without repeats; the scenario folders come from the table.
/// Classifying itself does not fail: the errors come from reading the zone map and the table.
pub fn classify(
    zones: &ZoneMap,
    table: &RulesTable,
    change: &Change,
) -> Result<Classification, Error> {
    classify_with(zones, table, change, &[])
}

/// Classifies a change like [`classify`], knowing `features`: scenario files the change has
/// that its paths may not show, such as the ones G1 sealed (hidden examples among them). They
/// are among its scenario files, so they put it in G1, and a change in G1 takes the full lane.
pub fn classify_with(
    zones: &ZoneMap,
    table: &RulesTable,
    change: &Change,
    features: &[String],
) -> Result<Classification, Error> {
    let mut paths = change.paths.clone();
    paths.sort();
    paths.dedup();

    let mut reasons = Vec::new();
    let mut highest = None;
    for path in &paths {
        let (c, why) = match zones.zone_of(path) {
            Some(zone) => (zone.color, zone.reason.as_str()),
            None => (zones.default, "no zone: default color"),
        };
        reasons.push(format!("{path} -> {c} ({why})"));
        highest = highest.max(Some(c));
    }
    let mut color = match highest {
        Some(c) => c,
        None => {
            reasons.push("no paths: default color".to_string());
            zones.default
        }
    };
    if let Some(min) = change.min_color.filter(|&min| min > color) {
        color = min;
        reasons.push(format!("declared minimum color: {color}"));
    }

    let mut features: Vec<String> = paths
        .into_iter()
        .filter(|p| table.acceptance.needs_approval(p))
        .chain(features.iter().cloned())
        .collect();
    features.sort();
    features.dedup();
    let g1 = !features.is_empty();

    let lane = if color != Color::Red && !g1 && change.resumable {
        Lane::Fast
    } else {
        Lane::Full
    };
    let rule = table.rule(color, lane);
    Ok(Classification {
        color,
        lane,
        reasons,
        g1,
        features,
        tests_first: rule.tests_first.unwrap_or(g1) && (g1 || color != Color::Green),
        mutation: rule.mutation,
        reviewer: rule.reviewer,
        g4: color == Color::Red || (g1 && rule.g4_if_g1),
        human_plan: color == Color::Red,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules;

    fn change(paths: &[&str], resumable: bool) -> Change {
        Change {
            paths: paths.iter().map(|p| p.to_string()).collect(),
            min_color: None,
            resumable,
        }
    }

    fn zones() -> ZoneMap {
        ZoneMap::from_toml(
            "default = \"yellow\"\n[[zone]]\ncolor = \"green\"\npaths = [\"docs/**\"]\n",
        )
        .unwrap()
    }

    #[test]
    fn green_fast_skips_everything() {
        let c = classify(
            &zones(),
            &rules::default_table(),
            &change(&["docs/a.md"], true),
        )
        .unwrap();
        assert_eq!((c.color, c.lane), (Color::Green, Lane::Fast));
        assert!(!c.tests_first && !c.mutation && !c.reviewer && !c.g1 && !c.g4);
    }

    #[test]
    fn green_with_scenarios_writes_tests_first() {
        let zones = ZoneMap::from_toml(
            "[[zone]]\ncolor = \"green\"\npaths = [\"docs/**\", \"features/**\"]\n",
        )
        .unwrap();
        let c = classify(
            &zones,
            &rules::default_table(),
            &change(&["docs/a.md", "features/a.feature"], true),
        )
        .unwrap();
        assert_eq!((c.color, c.lane), (Color::Green, Lane::Full));
        assert!(c.tests_first && c.g1 && c.g4 && c.reviewer);
        assert_eq!(c.features, vec!["features/a.feature".to_string()]);
    }

    #[test]
    fn yellow_without_scenarios_writes_tests_first_without_g4() {
        let ch = change(&["x.rs"], false);
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!((c.color, c.lane), (Color::Yellow, Lane::Full));
        assert!(c.tests_first && c.mutation && c.reviewer);
        assert!(!c.g1 && !c.g4 && !c.human_plan);
    }

    #[test]
    fn red_travels_full_with_the_human() {
        let mut ch = change(&["x.rs"], true);
        ch.min_color = Some(Color::Red);
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!((c.color, c.lane), (Color::Red, Lane::Full));
        assert!(c.tests_first && c.g4 && c.human_plan);
        assert!(!c.g1);
    }

    #[test]
    fn the_rule_decides_whether_tests_go_first() {
        let table = RulesTable::from_toml("[color.yellow.full]\ntests_first = false\n").unwrap();
        let c = classify(&zones(), &table, &change(&["x.rs"], false)).unwrap();
        assert_eq!(c.color, Color::Yellow);
        assert!(!c.tests_first);
    }

    #[test]
    fn a_minimum_equal_to_the_color_adds_no_reason() {
        let mut ch = change(&["x.rs"], true);
        ch.min_color = Some(Color::Yellow);
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!(c.color, Color::Yellow);
        assert_eq!(
            c.reasons,
            vec!["x.rs -> yellow (no zone: default color)".to_string()]
        );
    }

    #[test]
    fn minimum_color_never_lowers() {
        let mut ch = change(&["docs/a.md"], true);
        ch.min_color = Some(Color::Yellow);
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!(c.color, Color::Yellow);
        ch.min_color = Some(Color::Green);
        ch.paths = vec!["x.rs".into()];
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!(c.color, Color::Yellow);
    }

    #[test]
    fn reasons_follow_sorted_paths_without_repeats() {
        let zones = ZoneMap::from_toml(
            "[[zone]]\ncolor = \"green\"\npaths = [\"docs/**\"]\nreason = \"Documentation\"\n",
        )
        .unwrap();
        let ch = change(&["x.rs", "docs/a.md", "x.rs"], true);
        let c = classify(&zones, &rules::default_table(), &ch).unwrap();
        assert_eq!(
            c.reasons,
            vec![
                "docs/a.md -> green (Documentation)".to_string(),
                "x.rs -> yellow (no zone: default color)".to_string(),
            ]
        );
    }

    #[test]
    fn no_paths_and_a_raised_minimum_are_explained() {
        let mut ch = change(&[], true);
        ch.min_color = Some(Color::Red);
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!(
            c.reasons,
            vec![
                "no paths: default color".to_string(),
                "declared minimum color: red".to_string(),
            ]
        );
    }

    #[test]
    fn a_path_in_a_lower_zone_lowers_the_default() {
        let c = classify(
            &zones(),
            &rules::default_table(),
            &change(&["docs/a.md"], false),
        )
        .unwrap();
        assert_eq!((c.color, c.lane), (Color::Green, Lane::Full));
        // Green writes tests first only with new scenarios, whatever its rule says.
        assert!(!c.tests_first);
    }

    #[test]
    fn a_table_without_the_color_takes_the_defaults() {
        let empty = RulesTable::from_toml("").unwrap();
        let c = classify(&zones(), &empty, &change(&["x.rs"], false)).unwrap();
        assert_eq!((c.color, c.lane), (Color::Yellow, Lane::Full));
        assert!(c.reviewer);
        assert!(!c.tests_first && !c.mutation && !c.g1 && !c.g4);
        let c = classify(
            &zones(),
            &empty,
            &change(&["x.rs", "features/a.feature"], false),
        )
        .unwrap();
        assert!(c.g1 && c.tests_first && c.g4 && c.reviewer);
        assert!(!c.mutation);
    }

    #[test]
    fn the_scenario_folders_come_from_the_rules_table() {
        let table = RulesTable::from_toml("[acceptance]\ndir = \"specs\"\n").unwrap();
        let c = classify(&zones(), &table, &change(&["specs/a.feature"], true)).unwrap();
        assert_eq!((c.color, c.lane), (Color::Yellow, Lane::Full));
        assert!(c.g1 && c.g4);
        assert_eq!(c.features, vec!["specs/a.feature".to_string()]);
        let c = classify(&zones(), &table, &change(&["features/a.feature"], true)).unwrap();
        assert_eq!(c.lane, Lane::Fast);
        assert!(!c.g1 && c.features.is_empty());
    }

    #[test]
    fn scenarios_the_change_already_has_put_it_in_g1() {
        let table = rules::default_table();
        let ch = change(&["x.rs", "features/b.feature"], true);
        let sealed = [
            "features/hidden/a.feature".to_string(),
            "features/b.feature".to_string(),
            "features/hidden/a.feature".to_string(),
        ];
        let c = classify_with(&zones(), &table, &ch, &sealed).unwrap();
        assert_eq!((c.color, c.lane), (Color::Yellow, Lane::Full));
        assert!(c.g1 && c.g4 && c.tests_first);
        let features = ["features/b.feature", "features/hidden/a.feature"];
        assert_eq!(c.features, features);
        // They are scenarios, not paths: no reason, no color.
        let plain = change(&["docs/a.md"], true);
        let c = classify_with(&zones(), &table, &plain, &sealed[..1]).unwrap();
        assert_eq!((c.color, c.lane), (Color::Green, Lane::Full));
        assert!(c.g1 && c.g4 && c.tests_first);
        assert_eq!(c.reasons, ["docs/a.md -> green ()"]);
        assert_eq!(c.features, ["features/hidden/a.feature"]);
        let c = classify_with(&zones(), &table, &plain, &[]).unwrap();
        assert_eq!(c, classify(&zones(), &table, &plain).unwrap());
        assert_eq!(c.lane, Lane::Fast);
        assert!(!c.g1 && !c.g4);
    }

    #[test]
    fn as_json_the_color_and_the_lane_are_their_names() {
        let c = classify(
            &zones(),
            &rules::default_table(),
            &change(&["docs/a.md"], true),
        )
        .unwrap();
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["color"], "green");
        assert_eq!(json["lane"], "fast");
        assert_eq!(json["reasons"][0], "docs/a.md -> green ()");
        assert_eq!(json["g4"], false);
    }
}
