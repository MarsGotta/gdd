//! Classifying a change: its color, the lane it travels and the gates it must pass.

use std::fmt;

use serde::{Serialize, Serializer};

use crate::config::{Color, Error, Lane};
use crate::guard::Touched;
use crate::rules::{Depth, RulesTable};
use crate::rules::{Level, LockStep, MUTATION, is_core_item};
use crate::zones::ZoneMap;

/// The kind of work of a spike: it answers one question, and only its ADR reaches main.
pub const SPIKE: &str = "Spike";

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
    /// How deep the Reviewer looks: `"quick"` or `"deep"` as JSON.
    pub review: Depth,
    /// The human release gate (G4) applies.
    pub g4: bool,
    /// The human approves the plan.
    pub human_plan: bool,
}

fn as_text<T: fmt::Display, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// Why one of the core's own items is red when its zone leaves it lower.
const CORE_REASON: &str = "the core's own items: always in the most sensitive area";

/// The risk of a change, computed by the core from its paths and the approved zone map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Risk {
    /// Risk color.
    pub color: Color,
    /// Lane it travels.
    pub lane: Lane,
    /// Whether it brings a new lock: lock items that need the human's approval (G1).
    pub new_lock: bool,
    /// Why: one line per path, plus the declared minimum when it raised the color.
    pub reasons: Vec<String>,
    /// The lock items it brings, sorted and each once.
    pub features: Vec<String>,
}

/// The color of `path` and why: its zone's, or the default color. One of the core's own items
/// (see [`is_core_item`]) is red whatever its zone says: when the zone is already red, its
/// reason stays the zone's.
fn color_of<'a>(zones: &'a ZoneMap, path: &str) -> (Color, &'a str) {
    let (color, why) = match zones.zone_of(path) {
        Some(zone) => (zone.color, zone.reason.as_str()),
        None => (zones.default, "no zone: default color"),
    };
    if color < Color::Red && is_core_item(path) {
        return (Color::Red, CORE_REASON);
    }
    (color, why)
}

/// What the core decided for a change: its risk and the level of its gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Levels {
    /// The risk it was decided from.
    pub risk: Risk,
    /// The level of its gates.
    pub level: Level,
    /// The optional commands that run (see [`MUTATION`]).
    pub run: Vec<String>,
    /// How deep the guard looks.
    pub guard: Depth,
    /// Tests are written before the code.
    pub tests_first: bool,
    /// The human approves the plan.
    pub plan_approval: bool,
}

impl Levels {
    /// The levels `table` gives `risk` (see [`RulesTable::level_rule`]): the level of its rule,
    /// raised to the level for a new lock when it brings one; the tests first as the lock step
    /// says; and the human's approval of the plan in red.
    pub fn of(risk: Risk, table: &RulesTable) -> Levels {
        let rule = table.level_rule(risk.color, risk.lane);
        let level = match rule.new_lock {
            Some(new_lock) if risk.new_lock => rule.level.max(new_lock),
            _ => rule.level,
        };
        let tests_first = match rule.lock_step {
            LockStep::Always => true,
            LockStep::NewLock => risk.new_lock,
            LockStep::Never => false,
        };
        let plan_approval = risk.color == Color::Red;
        Levels {
            risk,
            level,
            run: rule.run,
            guard: rule.guard,
            tests_first,
            plan_approval,
        }
    }

    /// The decision as `tminus classify` prints it: the Reviewer from [`Level::LockGuard`] up,
    /// the human G4 at [`Level::LockGuardApproval`], mutation testing when it runs, and the rest
    /// from the risk.
    pub fn classification(self) -> Classification {
        let Levels {
            risk,
            level,
            run,
            guard,
            tests_first,
            plan_approval,
        } = self;
        let Risk {
            color,
            lane,
            new_lock,
            reasons,
            features,
        } = risk;
        Classification {
            color,
            lane,
            reasons,
            g1: new_lock,
            features,
            tests_first,
            mutation: run.iter().any(|command| command == MUTATION),
            reviewer: level >= Level::LockGuard,
            review: guard,
            g4: level == Level::LockGuardApproval,
            human_plan: plan_approval,
        }
    }
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
    let risk = risk(zones, table, change, features);
    Ok(Levels::of(risk, table).classification())
}

/// The risk of `change`: the highest color among its paths (or the default color when it has
/// none), raised to its declared minimum; its lock items, from its paths and `features`, the ones
/// it already has; and its lane, fast only when it is not red, brings no new lock and can be
/// resumed. Paths are taken sorted and without repeats; the folders of the lock items come from
/// `table`.
pub fn risk(zones: &ZoneMap, table: &RulesTable, change: &Change, features: &[String]) -> Risk {
    let mut paths = change.paths.clone();
    paths.sort();
    paths.dedup();

    let mut reasons = Vec::new();
    let mut highest = None;
    for path in &paths {
        let (c, why) = color_of(zones, path);
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
    let new_lock = !features.is_empty();

    let lane = if color != Color::Red && !new_lock && change.resumable {
        Lane::Fast
    } else {
        Lane::Full
    };
    Risk {
        color,
        lane,
        new_lock,
        reasons,
        features,
    }
}

/// Classifies the change `plan` like [`classify_with`], with the items `touched` reports among
/// its paths: what the change touched counts as much as what it plans, an item it removed too.
pub fn classify_touched(
    zones: &ZoneMap,
    table: &RulesTable,
    plan: &Change,
    touched: &Touched,
    features: &[String],
) -> Result<Classification, Error> {
    let mut change = plan.clone();
    change.paths.extend(touched.paths());
    classify_with(zones, table, &change, features)
}

/// `classification` for a change of the kind of work `kind`. A [`SPIKE`] keeps its color (and so
/// its Builder), but no test mode, no mutation testing, no Reviewer, no G1 and no plan for the
/// human to approve: nothing but its ADR reaches main, so neither its rules nor the core of red
/// apply; and the human G4 always. Any other kind keeps it as it is.
pub fn of_kind(classification: Classification, kind: &str) -> Classification {
    if kind != SPIKE {
        return classification;
    }
    Classification {
        g1: false,
        tests_first: false,
        mutation: false,
        reviewer: false,
        g4: true,
        human_plan: false,
        ..classification
    }
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
    fn what_the_change_touched_counts_like_what_it_plans() {
        let table = rules::default_table();
        let plan = change(&["docs/a.md"], true);
        let ids = ["features/b.feature", "x.rs", "docs/a.md"].map(String::from);
        let touched: Touched = ids.into_iter().collect();
        let sealed = ["features/hidden/a.feature".to_string()];
        let c = classify_touched(&zones(), &table, &plan, &touched, &sealed).unwrap();
        let joined = change(&["docs/a.md", "features/b.feature", "x.rs"], true);
        let expected = classify_with(&zones(), &table, &joined, &sealed).unwrap();
        assert_eq!(c, expected);
        assert_eq!((c.color, c.lane), (Color::Yellow, Lane::Full));
        let features = ["features/b.feature", "features/hidden/a.feature"];
        assert_eq!(c.features, features);
        let nothing = Touched::default();
        let c = classify_touched(&zones(), &table, &plan, &nothing, &[]).unwrap();
        assert_eq!(c, classify(&zones(), &table, &plan).unwrap());
        assert_eq!((c.color, c.lane), (Color::Green, Lane::Fast));
    }

    #[test]
    fn a_spike_keeps_its_color_and_waits_at_g4_without_tests_mutation_reviewer_g1_or_plan() {
        let table = rules::default_table();
        let mut red = change(&["x.rs", "features/a.feature"], false);
        red.min_color = Some(Color::Red);
        let c = classify(&zones(), &table, &red).unwrap();
        assert!(c.tests_first && c.mutation && c.reviewer && c.g1 && c.human_plan);
        let s = of_kind(c.clone(), SPIKE);
        assert_eq!((s.color, s.lane), (Color::Red, Lane::Full));
        assert!(!s.tests_first && !s.mutation && !s.reviewer && !s.g1);
        assert!(s.g4 && !s.human_plan);
        assert_eq!((&s.reasons, &s.features), (&c.reasons, &c.features));
        assert_eq!(s.review, c.review);
        // Green fast has no G4 of its own: a spike has it.
        let green = classify(&zones(), &table, &change(&["docs/a.md"], true)).unwrap();
        assert!(!green.g4);
        let s = of_kind(green, SPIKE);
        assert_eq!((s.color, s.lane), (Color::Green, Lane::Fast));
        assert!(s.g4 && !s.human_plan);
        // Other kinds keep their classification.
        for kind in ["Feature", "Refactor", "spike", ""] {
            assert_eq!(of_kind(c.clone(), kind), c, "{kind}");
        }
        assert_eq!(SPIKE, "Spike");
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
        assert_eq!(json["review"], "quick");
    }

    #[test]
    fn the_review_is_deep_for_red_and_quick_otherwise_unless_the_rules_say() {
        let mut red = change(&["x.rs"], false);
        red.min_color = Some(Color::Red);
        let (yellow, green) = (change(&["x.rs"], true), change(&["docs/a.md"], true));
        let shipped = rules::default_table();
        let review = |table: &RulesTable, ch: &Change| classify(&zones(), table, ch).unwrap();
        assert_eq!(review(&shipped, &red).review, Depth::Deep);
        assert_eq!(review(&shipped, &yellow).review, Depth::Quick);
        assert_eq!(review(&shipped, &green).review, Depth::Quick);
        let deep = RulesTable::from_toml("[color.yellow.fast]\nreview = \"deep\"\n").unwrap();
        let c = review(&deep, &yellow);
        assert_eq!((c.lane, c.review), (Lane::Fast, Depth::Deep));
        assert_eq!(review(&deep, &green).review, Depth::Quick);
        let json = serde_json::to_value(review(&deep, &red)).unwrap();
        assert_eq!(json["review"], "deep");
        let json = serde_json::to_value(review(&shipped, &yellow)).unwrap();
        assert_eq!(json["review"], "quick");
    }

    /// A lock item: a file of the folder whose items need the human's approval (G1).
    const LOCK: &str = "features/a.feature";

    /// Why one of the core's items is red, as a reason says it.
    const CORE: &str = "red (the core's own items: always in the most sensitive area)";

    /// A zone map with the three colors; the lock items are green.
    const COLORS: &str = "default = \"yellow\"\n\
                          [[zone]]\ncolor = \"green\"\npaths = [\"docs/**\", \"features/**\"]\n\
                          [[zone]]\ncolor = \"red\"\npaths = [\"src/storage/**\"]\n";

    /// One rule a color in a lane can write: `tests_first`, `mutation`, `reviewer`, `g4_if_g1`
    /// and whether the review is deep.
    type Keys = (Option<bool>, bool, bool, bool, bool);

    /// Every rule a color in a lane can write.
    fn every_rule() -> Vec<Keys> {
        let mut all = Vec::new();
        for tests_first in [None, Some(true), Some(false)] {
            for bits in 0..16u8 {
                let on = |n: u8| bits & (1 << n) != 0;
                all.push((tests_first, on(0), on(1), on(2), on(3)));
            }
        }
        all
    }

    /// `keys` as the TOML table `[color.<table>]`. A red table leaves out a key that would
    /// switch off the core, as a rules file must.
    fn written(table: &str, keys: Keys) -> String {
        let (tests_first, mutation, reviewer, g4_if_g1, deep) = keys;
        let red = table.starts_with("red");
        let mut text = format!("[color.{table}]\n");
        if let Some(first) = tests_first {
            text += &format!("tests_first = {first}\n");
        }
        let gates = [
            ("mutation", mutation),
            ("reviewer", reviewer),
            ("g4_if_g1", g4_if_g1),
        ];
        for (key, on) in gates {
            if on || !red {
                text += &format!("{key} = {on}\n");
            }
        }
        if deep || !red {
            let depth = if deep { "deep" } else { "quick" };
            text += &format!("review = \"{depth}\"\n");
        }
        text
    }

    /// What `classify` decided before levels, copied: the lane and the gates of `classify_with`
    /// and red's `mutation` and `review` forced on as `RulesTable::rule` did. The color, the
    /// reasons and the lock items come from `risk`, which keeps that half as it was.
    fn decided_before_levels(table: &RulesTable, change: &Change, risk: &Risk) -> Classification {
        let Risk {
            color,
            reasons,
            features,
            ..
        } = risk.clone();
        let g1 = !features.is_empty();
        let lane = if color != Color::Red && !g1 && change.resumable {
            Lane::Fast
        } else {
            Lane::Full
        };
        let mut rule = table.rule(color, lane);
        if color == Color::Red {
            rule.mutation = true;
            rule.review = Depth::Deep;
        }
        Classification {
            color,
            lane,
            reasons,
            g1,
            features,
            tests_first: rule.tests_first.unwrap_or(g1) && (g1 || color != Color::Green),
            mutation: rule.mutation,
            reviewer: rule.reviewer,
            review: rule.review,
            g4: color == Color::Red || (g1 && rule.g4_if_g1),
            human_plan: color == Color::Red,
        }
    }

    /// The tables of the equivalence: the one of the tool, an empty one, and one per rule of
    /// [`every_rule`], each lane of green and yellow with a different rule.
    fn every_table() -> Vec<RulesTable> {
        let rules = every_rule();
        let mut tables = vec![rules::default_table(), RulesTable::from_toml("").unwrap()];
        for (&keys, &other) in rules.iter().zip(rules.iter().rev()) {
            let text = [
                written("green.fast", keys),
                written("green.full", other),
                written("yellow.fast", other),
                written("yellow.full", keys),
                written("red.full", keys),
            ]
            .concat();
            tables.push(RulesTable::from_toml(&text).unwrap());
        }
        tables
    }

    #[test]
    fn the_levels_decide_as_before_except_that_no_approval_comes_without_the_guard() {
        let zones = ZoneMap::from_toml(COLORS).unwrap();
        let colors = [
            ("docs/a.md", Color::Green),
            ("x.rs", Color::Yellow),
            ("src/storage/db.rs", Color::Red),
        ];
        let cases = [(false, false), (false, true), (true, false), (true, true)];
        let mut tightened = 0;
        for (n, table) in every_table().iter().enumerate() {
            for (path, color) in colors {
                for (lock, resumable) in cases {
                    let paths = if lock { vec![path, LOCK] } else { vec![path] };
                    let ch = change(&paths, resumable);
                    let c = classify(&zones, table, &ch).unwrap();
                    let found = risk(&zones, table, &ch, &[]);
                    let mut before = decided_before_levels(table, &ch, &found);
                    // The one tightening: the human's approval brings the guard with it.
                    if before.g4 && !before.reviewer {
                        before.reviewer = true;
                        tightened += 1;
                    }
                    assert_eq!(c, before, "table {n}, {paths:?}, resumable {resumable}");
                    assert_eq!((c.color, c.g1), (color, lock), "table {n}, {paths:?}");
                }
            }
        }
        assert!(tightened > 0, "no rule left the guard out of an approval");
    }

    #[test]
    fn the_core_items_are_red_whatever_their_zone_says() {
        let table = rules::default_table();
        let items = [
            "tminus/rules.toml",
            ".tminus/changes/c1.json",
            "gdd/zones.toml",
            ".gdd/x",
        ];
        let zone = |color: &str| {
            let paths = rules::CORE_ITEMS.map(|p| format!("\"{p}\"")).join(", ");
            let text = format!("[[zone]]\ncolor = \"{color}\"\npaths = [{paths}]\n");
            text + "reason = \"Settings\"\n"
        };
        let maps = [
            zone("green"),
            zone("yellow"),
            "default = \"green\"\n".to_string(),
            "default = \"yellow\"\n".to_string(),
        ];
        for map in &maps {
            let zones = ZoneMap::from_toml(map).unwrap();
            for item in items {
                let c = classify(&zones, &table, &change(&[item], true)).unwrap();
                assert_eq!((c.color, c.lane), (Color::Red, Lane::Full), "{item}: {map}");
                assert_eq!(c.reasons, [format!("{item} -> {CORE}")], "{map}");
                let gates = c.reviewer && c.g4 && c.mutation && c.human_plan;
                assert!(gates, "{item}: {map}");
                assert_eq!(c.review, Depth::Deep);
            }
        }
        // Other names, or the core's names somewhere else, keep their zone's color.
        let zones = ZoneMap::from_toml("default = \"green\"\n").unwrap();
        for path in ["tminus.md", "src/gdd/a.rs", "gdd", "docs/tminus/a.md"] {
            let c = classify(&zones, &table, &change(&[path], true)).unwrap();
            assert_eq!((c.color, c.lane), (Color::Green, Lane::Fast), "{path}");
            let reason = format!("{path} -> green (no zone: default color)");
            assert_eq!(c.reasons, [reason]);
        }
    }

    #[test]
    fn a_core_item_already_red_keeps_the_reason_of_its_zone() {
        let text = "[[zone]]\ncolor = \"red\"\npaths = [\"gdd/**\"]\nreason = \"The harness\"\n";
        let zones = ZoneMap::from_toml(text).unwrap();
        let table = rules::default_table();
        let ch = change(&["tminus/a.toml", "gdd/rules.toml"], false);
        let c = classify(&zones, &table, &ch).unwrap();
        let reasons = [
            "gdd/rules.toml -> red (The harness)".to_string(),
            format!("tminus/a.toml -> {CORE}"),
        ];
        assert_eq!(c.reasons, reasons);
        let red = ZoneMap::from_toml("default = \"red\"\n").unwrap();
        let c = classify(&red, &table, &change(&["tminus/a.toml"], true)).unwrap();
        let reason = "tminus/a.toml -> red (no zone: default color)";
        assert_eq!(c.reasons, [reason]);
    }

    /// The risk of a change of `color` in `lane`, with a new lock or not.
    fn risk_of(color: Color, lane: Lane, new_lock: bool) -> Risk {
        Risk {
            color,
            lane,
            new_lock,
            reasons: vec!["why".to_string()],
            features: new_lock.then(|| LOCK.to_string()).into_iter().collect(),
        }
    }

    #[test]
    fn a_new_lock_raises_the_level_to_the_one_its_rule_gives_it() {
        let table = rules::default_table();
        let yellow = Levels::of(risk_of(Color::Yellow, Lane::Full, false), &table);
        assert_eq!(yellow.level, Level::LockGuard);
        assert_eq!((yellow.tests_first, yellow.plan_approval), (true, false));
        assert_eq!(yellow.run, [MUTATION]);
        assert_eq!(yellow.guard, Depth::Quick);
        assert_eq!(yellow.risk, risk_of(Color::Yellow, Lane::Full, false));
        let with_lock = Levels::of(risk_of(Color::Yellow, Lane::Full, true), &table);
        assert_eq!(with_lock.level, Level::LockGuardApproval);
        // Green writes the tests first only with a new lock; in its fast lane, the lock alone.
        let green = Levels::of(risk_of(Color::Green, Lane::Full, false), &table);
        assert_eq!((green.level, green.tests_first), (Level::LockGuard, false));
        let green = Levels::of(risk_of(Color::Green, Lane::Full, true), &table);
        assert_eq!(green.level, Level::LockGuardApproval);
        assert!(green.tests_first);
        let fast = Levels::of(risk_of(Color::Green, Lane::Fast, false), &table);
        assert_eq!((fast.level, fast.tests_first), (Level::Lock, false));
        assert!(fast.run.is_empty());
        // Red: the floor, and the human approves the plan.
        let red = Levels::of(risk_of(Color::Red, Lane::Full, false), &table);
        assert_eq!(red.level, Level::LockGuardApproval);
        assert_eq!(red.guard, Depth::Deep);
        assert!(red.plan_approval);
    }

    #[test]
    fn the_classification_reads_the_levels() {
        let levels = |level, run: &[&str]| Levels {
            risk: risk_of(Color::Yellow, Lane::Full, true),
            level,
            run: run.iter().map(|r| r.to_string()).collect(),
            guard: Depth::Deep,
            tests_first: true,
            plan_approval: false,
        };
        let gates = |level, run: &[&str]| {
            let c = levels(level, run).classification();
            (c.reviewer, c.g4, c.mutation)
        };
        assert_eq!(gates(Level::Open, &[]), (false, false, false));
        assert_eq!(gates(Level::Lock, &["lint"]), (false, false, false));
        assert_eq!(gates(Level::LockGuard, &[MUTATION]), (true, false, true));
        assert_eq!(gates(Level::LockGuardApproval, &[]), (true, true, false));
        let expected = Classification {
            color: Color::Yellow,
            lane: Lane::Full,
            reasons: vec!["why".to_string()],
            g1: true,
            features: vec![LOCK.to_string()],
            tests_first: true,
            mutation: false,
            reviewer: false,
            review: Depth::Deep,
            g4: false,
            human_plan: false,
        };
        assert_eq!(levels(Level::Lock, &[]).classification(), expected);
    }
}
