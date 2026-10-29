//! The rules table: which gates each color and lane asks for, and where the scenarios live.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::config::{Color, Error, Lane};

/// What one color in one lane asks for. A field the rules file leaves out takes the value of
/// [`Rule::default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// Tests are written before the code. Unset: only when the change has new scenarios (G1).
    pub tests_first: Option<bool>,
    /// Mutation testing runs. Unset: no.
    pub mutation: bool,
    /// The Reviewer checks the change. Unset: yes.
    pub reviewer: bool,
    /// New scenarios also need the human release gate (G4). Unset: yes.
    pub g4_if_g1: bool,
}

impl Default for Rule {
    /// The rule of a color the table does not mention, and the value of every field left out.
    fn default() -> Rule {
        Rule {
            tests_first: None,
            mutation: false,
            reviewer: true,
            g4_if_g1: true,
        }
    }
}

/// The rules of one color: the fields of `[color.<color>]` and an optional table per lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct ColorRules {
    #[serde(flatten)]
    rule: Rule,
    fast: Option<Rule>,
    full: Option<Rule>,
}

/// Where the acceptance scenarios live: the `[acceptance]` section of the rules file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Acceptance {
    /// Folder of the scenario files (`.feature`). Unset: `features`.
    pub dir: String,
    /// Folder whose scenarios do not need approval (G1). Unset: `features/regression`.
    pub regression: String,
}

impl Default for Acceptance {
    fn default() -> Acceptance {
        Acceptance {
            dir: "features".to_string(),
            regression: "features/regression".to_string(),
        }
    }
}

impl Acceptance {
    /// Whether `path` is a scenario file that needs approval (G1): a `.feature` file inside
    /// [`dir`](Acceptance::dir) and not inside [`regression`](Acceptance::regression).
    pub fn needs_approval(&self, path: &str) -> bool {
        path.ends_with(".feature") && inside(path, &self.dir) && !inside(path, &self.regression)
    }
}

/// Whether `path` is inside `folder`.
fn inside(path: &str, folder: &str) -> bool {
    path.strip_prefix(folder)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// The rules for every color and lane, and where the scenarios live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesTable {
    colors: BTreeMap<Color, ColorRules>,
    /// Where the acceptance scenarios live.
    pub acceptance: Acceptance,
}

#[derive(Deserialize)]
struct RawTable {
    color: Option<BTreeMap<String, ColorRules>>,
    #[serde(default)]
    acceptance: Acceptance,
}

impl RulesTable {
    /// Reads the `[color.*]` and `[acceptance]` sections of a rules file; the rest is ignored.
    /// A color takes `[color.<color>.<lane>]` or `[color.<color>]`; what it leaves out takes
    /// its default (see [`RulesTable::rule`]). An unknown color is an error.
    pub fn from_toml(text: &str) -> Result<RulesTable, Error> {
        RulesTable::read(text).map(|(table, _)| table)
    }

    /// The table a project classifies its changes with, from its rules file: the one the file
    /// writes or, when it has no `[color]` section, the colors of the table that ships with the
    /// tool ([`default_table`]) with the file's `[acceptance]`: saying nothing about colors
    /// switches no gate off. `gdd classify` and the gates share it.
    pub fn of_project(text: &str) -> Result<RulesTable, Error> {
        let (table, has_colors) = RulesTable::read(text)?;
        if has_colors {
            return Ok(table);
        }
        Ok(RulesTable {
            colors: default_table().colors,
            acceptance: table.acceptance,
        })
    }

    /// The table a rules file writes, and whether it has a `[color]` section. A file that
    /// switches off the core for red changes is refused (see [`core_switched_off`]).
    fn read(text: &str) -> Result<(RulesTable, bool), Error> {
        let raw: RawTable = toml::from_str(text).map_err(|e| Error::Toml(e.to_string()))?;
        let has_colors = raw.color.is_some();
        let colors = raw
            .color
            .unwrap_or_default()
            .into_iter()
            .map(|(name, rules)| Ok((name.parse()?, rules)))
            .collect::<Result<_, Error>>()?;
        if let Some(key) = core_switched_off(text)? {
            return Err(Error::CoreSwitchedOff(key));
        }
        let table = RulesTable {
            colors,
            acceptance: raw.acceptance,
        };
        Ok((table, has_colors))
    }

    /// The rule for a color in a lane: the lane's own table, else the color's table, else
    /// [`Rule::default`] when the table does not mention the color.
    pub fn rule(&self, color: Color, lane: Lane) -> Rule {
        self.colors.get(&color).map_or_else(Rule::default, |c| {
            match lane {
                Lane::Fast => c.fast,
                Lane::Full => c.full,
            }
            .unwrap_or(c.rule)
        })
    }
}

/// What a red change always gets, the core of GDD, as keys of a lane, in the order they are
/// reported: no rules file can set them to `false` for red.
const CORE: [&str; 3] = ["mutation", "reviewer", "g4_if_g1"];

/// The first key of the [`CORE`] the rules file `text` sets to `false` in a red lane, as
/// `color.red.<lane>.<key>`: lane `fast` before `full`. A key left out is not `false`.
fn core_switched_off(text: &str) -> Result<Option<String>, Error> {
    let file: toml::Table = toml::from_str(text).map_err(|e| Error::Toml(e.to_string()))?;
    let red = file.get("color").and_then(|colors| colors.get("red"));
    let switched_off = |&(lane, key): &(Lane, &str)| {
        let rule = red.and_then(|red| red.get(lane.to_string().as_str()));
        let value = rule.and_then(|rule| rule.get(key));
        value.and_then(toml::Value::as_bool) == Some(false)
    };
    let mut keys = [Lane::Fast, Lane::Full]
        .into_iter()
        .flat_map(|lane| CORE.map(|key| (lane, key)));
    let found = keys.find(switched_off);
    Ok(found.map(|(lane, key)| format!("color.red.{lane}.{key}")))
}

/// The table that ships with the tool; it equals the `[color.*]` and `[acceptance]` sections
/// of `gdd/rules.toml`. Red has no fast lane: it always travels the full one.
pub fn default_table() -> RulesTable {
    let rule = |tests_first, mutation, reviewer, g4_if_g1| Rule {
        tests_first: Some(tests_first),
        mutation,
        reviewer,
        g4_if_g1,
    };
    let lanes = |fast, full| ColorRules {
        rule: Rule::default(),
        fast,
        full,
    };
    RulesTable {
        colors: BTreeMap::from([
            (
                Color::Green,
                lanes(
                    Some(rule(false, false, false, false)),
                    Some(rule(true, false, true, true)),
                ),
            ),
            (
                Color::Yellow,
                lanes(
                    Some(rule(true, true, true, false)),
                    Some(rule(true, true, true, true)),
                ),
            ),
            (Color::Red, lanes(None, Some(rule(true, true, true, true)))),
        ]),
        acceptance: Acceptance::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule of a color or a field the table leaves out, spelled out.
    const UNSET: Rule = Rule {
        tests_first: None,
        mutation: false,
        reviewer: true,
        g4_if_g1: true,
    };

    fn table(text: &str) -> RulesTable {
        RulesTable::from_toml(text).unwrap()
    }

    #[test]
    fn default_table_equals_the_rules_file() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../gdd/rules.toml");
        let text = std::fs::read_to_string(path).expect("gdd/rules.toml");
        assert_eq!(RulesTable::from_toml(&text), Ok(default_table()));
    }

    #[test]
    fn default_table_lanes() {
        let t = default_table();
        assert_eq!(
            t.rule(Color::Green, Lane::Fast),
            Rule {
                tests_first: Some(false),
                mutation: false,
                reviewer: false,
                g4_if_g1: false,
            }
        );
        assert_eq!(
            t.rule(Color::Red, Lane::Full),
            Rule {
                tests_first: Some(true),
                mutation: true,
                reviewer: true,
                g4_if_g1: true,
            }
        );
    }

    #[test]
    fn fields_left_out_take_their_default() {
        let t = table("[color.yellow.full]\ntests_first = true\n");
        assert_eq!(
            t.rule(Color::Yellow, Lane::Full),
            Rule {
                tests_first: Some(true),
                ..UNSET
            }
        );
        assert_eq!(Rule::default(), UNSET);
    }

    #[test]
    fn a_lane_without_its_own_table_follows_the_color() {
        let t = table("[color.yellow]\nmutation = true\n[color.yellow.full]\nreviewer = false\n");
        assert_eq!(
            t.rule(Color::Yellow, Lane::Fast),
            Rule {
                mutation: true,
                ..UNSET
            }
        );
        // The lane's own table replaces the color's table; it does not inherit from it.
        assert_eq!(
            t.rule(Color::Yellow, Lane::Full),
            Rule {
                reviewer: false,
                ..UNSET
            }
        );
    }

    #[test]
    fn a_color_left_out_takes_the_defaults() {
        let t = table("");
        for color in [Color::Green, Color::Yellow, Color::Red] {
            for lane in [Lane::Fast, Lane::Full] {
                assert_eq!(t.rule(color, lane), UNSET);
            }
        }
    }

    #[test]
    fn other_lanes_and_keys_are_ignored() {
        let t = table("[color.red.slow]\nmutation = true\n[color.red.full]\nnote = \"x\"\n");
        assert_eq!(t.rule(Color::Red, Lane::Full), UNSET);
        assert_eq!(t.rule(Color::Red, Lane::Fast), UNSET);
    }

    #[test]
    fn unknown_colors_are_errors() {
        assert_eq!(
            RulesTable::from_toml("[color.purple.full]\nmutation = true\n"),
            Err(Error::UnknownColor("purple".into()))
        );
        assert_eq!(
            RulesTable::from_toml("[color.purple]\n"),
            Err(Error::UnknownColor("purple".into()))
        );
    }

    #[test]
    fn a_project_without_colors_classifies_with_the_table_of_the_tool() {
        let shipped = default_table();
        assert_eq!(RulesTable::of_project(""), Ok(shipped.clone()));
        let text = "[protected]\nall = [\"gdd/**\"]\n[acceptance]\ndir = \"specs\"\n";
        let project = RulesTable::of_project(text).unwrap();
        assert_eq!(project.acceptance.dir, "specs");
        assert_eq!(project.colors, shipped.colors);
        // With colors of its own, or an empty [color] section, the file decides.
        let own = "[color.yellow.full]\nmutation = false\n";
        assert_eq!(RulesTable::of_project(own), RulesTable::from_toml(own));
        assert_eq!(RulesTable::of_project("[color]\n"), Ok(table("")));
        assert_eq!(
            RulesTable::of_project("[color.purple]\n"),
            Err(Error::UnknownColor("purple".into()))
        );
        let wrong = RulesTable::of_project("color = 1\n");
        assert!(matches!(wrong, Err(Error::Toml(_))), "{wrong:?}");
    }

    /// The error of a rules file that switches `key` off for red changes.
    fn core_off(key: &str) -> Result<RulesTable, Error> {
        Err(Error::CoreSwitchedOff(key.into()))
    }

    #[test]
    fn no_red_lane_can_switch_off_the_core() {
        for lane in ["fast", "full"] {
            for key in ["mutation", "reviewer", "g4_if_g1"] {
                let text = format!("[color.red.{lane}]\n{key} = false\n");
                let expected = core_off(&format!("color.red.{lane}.{key}"));
                assert_eq!(RulesTable::from_toml(&text), expected, "{text}");
                assert_eq!(RulesTable::of_project(&text), expected, "{text}");
            }
        }
        let said = Error::CoreSwitchedOff("color.red.full.mutation".into()).to_string();
        let expected = "red changes always get mutation testing, the Reviewer and the human G4 \
                        (the core of GDD): remove color.red.full.mutation = false from \
                        gdd/rules.toml";
        assert_eq!(said, expected);
    }

    #[test]
    fn the_first_key_switched_off_is_named_lane_fast_first_then_by_key() {
        let read = RulesTable::from_toml;
        let both = "[color.red.full]\nmutation = false\n[color.red.fast]\ng4_if_g1 = false\n";
        assert_eq!(read(both), core_off("color.red.fast.g4_if_g1"));
        let keys = "[color.red.full]\ng4_if_g1 = false\nreviewer = false\nmutation = false\n";
        assert_eq!(read(keys), core_off("color.red.full.mutation"));
        let last = "[color.red.full]\ng4_if_g1 = false\nreviewer = false\nmutation = true\n";
        assert_eq!(read(last), core_off("color.red.full.reviewer"));
    }

    #[test]
    fn a_red_lane_that_leaves_the_core_out_keeps_the_defaults() {
        let t = table("[color.red.full]\ntests_first = false\n");
        let expected = Rule {
            tests_first: Some(false),
            ..UNSET
        };
        assert_eq!(t.rule(Color::Red, Lane::Full), expected);
        let on = "[color.red.full]\nmutation = true\nreviewer = true\ng4_if_g1 = true\n";
        let expected = Rule {
            mutation: true,
            ..UNSET
        };
        assert_eq!(table(on).rule(Color::Red, Lane::Full), expected);
    }

    #[test]
    fn false_stays_allowed_outside_red() {
        let off = "mutation = false\nreviewer = false\ng4_if_g1 = false\n";
        let text = format!(
            "[color.green.fast]\n{off}[color.yellow]\n{off}[color.yellow.full]\n{off}\
             [color.red.slow]\n{off}"
        );
        let t = table(&text);
        let none = Rule {
            mutation: false,
            reviewer: false,
            g4_if_g1: false,
            ..UNSET
        };
        assert_eq!(t.rule(Color::Green, Lane::Fast), none);
        assert_eq!(t.rule(Color::Yellow, Lane::Fast), none);
        assert_eq!(t.rule(Color::Yellow, Lane::Full), none);
    }

    #[test]
    fn a_field_that_is_not_a_boolean_is_an_error() {
        assert!(matches!(
            RulesTable::from_toml("[color.red.full]\nmutation = \"yes\"\n"),
            Err(Error::Toml(_))
        ));
    }

    #[test]
    fn acceptance_folders_come_from_the_rules_file() {
        let defaults = table("").acceptance;
        assert_eq!(defaults.dir, "features");
        assert_eq!(defaults.regression, "features/regression");
        let t = table("[acceptance]\ndir = \"specs\"\nrequired = true\n");
        assert_eq!(t.acceptance.dir, "specs");
        assert_eq!(t.acceptance.regression, "features/regression");
        let t = table("[acceptance]\ndir = \"specs\"\nregression = \"specs/old\"\n");
        assert_eq!(t.acceptance.regression, "specs/old");
    }

    #[test]
    fn only_scenarios_outside_regression_need_approval() {
        let a = Acceptance::default();
        assert!(a.needs_approval("features/a.feature"));
        assert!(a.needs_approval("features/sub/a.feature"));
        assert!(!a.needs_approval("features/regression/a.feature"));
        assert!(!a.needs_approval("features/a.txt"));
        assert!(!a.needs_approval("other/a.feature"));
        assert!(!a.needs_approval("features2/a.feature"));
        assert!(!a.needs_approval("features.feature"));
    }

    #[test]
    fn scenario_folders_follow_the_acceptance_section() {
        let a = Acceptance {
            dir: "specs".into(),
            regression: "specs/regression".into(),
        };
        assert!(a.needs_approval("specs/a.feature"));
        assert!(!a.needs_approval("specs/regression/a.feature"));
        assert!(!a.needs_approval("features/a.feature"));
    }
}
