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
    #[serde(default)]
    color: BTreeMap<String, ColorRules>,
    #[serde(default)]
    acceptance: Acceptance,
}

impl RulesTable {
    /// Reads the `[color.*]` and `[acceptance]` sections of a rules file; the rest is ignored.
    /// A color takes `[color.<color>.<lane>]` or `[color.<color>]`; what it leaves out takes
    /// its default (see [`RulesTable::rule`]). An unknown color is an error.
    pub fn from_toml(text: &str) -> Result<RulesTable, Error> {
        let raw: RawTable = toml::from_str(text).map_err(|e| Error::Toml(e.to_string()))?;
        let colors = raw
            .color
            .into_iter()
            .map(|(name, rules)| Ok((name.parse()?, rules)))
            .collect::<Result<_, Error>>()?;
        Ok(RulesTable {
            colors,
            acceptance: raw.acceptance,
        })
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
