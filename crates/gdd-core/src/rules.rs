//! The rules table: which gates each color and lane asks for, and where the scenarios live; the
//! budget of a spike; and the time limit of the gate and release commands.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{Color, Error, Lane};
use crate::exec;

/// What the agent calls of a spike may cost, in USD, when the rules file says nothing.
pub const SPIKE_USD: f64 = 5.0;

/// How long each gate and release command may run: `[limits] command_seconds` of a rules file,
/// [`exec::TIMEOUT`] when it says nothing. The rest of the file is ignored. A value that is not
/// a whole number above 0 is an error that names `limits.command_seconds`.
pub fn command_seconds(text: &str) -> Result<Duration, Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Raw {
        limits: RawLimits,
    }
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct RawLimits {
        command_seconds: Option<toml::Value>,
    }
    let raw: Raw = toml::from_str(text).map_err(|e| Error::Toml(e.to_string()))?;
    let Some(value) = raw.limits.command_seconds else {
        return Ok(exec::TIMEOUT);
    };
    let seconds = value.as_integer().and_then(|n| u64::try_from(n).ok());
    match seconds.filter(|&seconds| seconds > 0) {
        Some(seconds) => Ok(Duration::from_secs(seconds)),
        None => Err(Error::Toml(format!(
            "limits.command_seconds must be a whole number of seconds above 0, not {value}"
        ))),
    }
}

/// The budget of a spike: what its agent calls may cost, in USD, before it parks; `[spike]
/// max_usd` of a rules file, [`SPIKE_USD`] when it says nothing. The rest of the file is ignored.
/// A value that is not a positive number is an error that names `spike.max_usd`.
pub fn spike_budget(text: &str) -> Result<f64, Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Raw {
        spike: RawSpike,
    }
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct RawSpike {
        max_usd: Option<toml::Value>,
    }
    let raw: Raw = toml::from_str(text).map_err(|e| Error::Toml(e.to_string()))?;
    let Some(value) = raw.spike.max_usd else {
        return Ok(SPIKE_USD);
    };
    let usd = match value {
        toml::Value::Integer(whole) => whole as f64,
        toml::Value::Float(usd) => usd,
        _ => f64::NAN,
    };
    if usd > 0.0 && usd.is_finite() {
        Ok(usd)
    } else {
        Err(Error::Toml(format!(
            "spike.max_usd must be a positive number of USD, not {value}"
        )))
    }
}

/// What one color in one lane asks for. A field the rules file leaves out takes the value of
/// [`Rule::default`], except `mutation` and `review` for red (see [`RulesTable::rule`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// Tests are written before the code. Unset: only when the change has new scenarios (G1).
    pub tests_first: Option<bool>,
    /// Mutation testing runs. Unset: no, except for red.
    pub mutation: bool,
    /// The Reviewer checks the change. Unset: yes.
    pub reviewer: bool,
    /// New scenarios also need the human release gate (G4). Unset: yes.
    pub g4_if_g1: bool,
    /// How deep the Reviewer looks. Unset: quick, except for red.
    pub review: Depth,
}

impl Default for Rule {
    /// The rule of a color the table does not mention, and the value of every field left out.
    fn default() -> Rule {
        Rule {
            tests_first: None,
            mutation: false,
            reviewer: true,
            g4_if_g1: true,
            review: Depth::Quick,
        }
    }
}

/// How deep the Reviewer looks at a change. As TOML and JSON, its name: `"quick"`, `"deep"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Depth {
    /// The files the change touched, its package and its scenarios.
    Quick,
    /// The quick review, plus the code around the change and what can go badly wrong there.
    Deep,
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

    /// The table a rules file writes, and whether it has a `[color]` section. A review depth
    /// other than quick or deep is refused (see [`unknown_depth`]), and so is a file that
    /// switches off the core for red changes (see [`core_switched_off`]).
    fn read(text: &str) -> Result<(RulesTable, bool), Error> {
        let toml_error = |e: toml::de::Error| Error::Toml(e.to_string());
        let file: toml::Table = toml::from_str(text).map_err(toml_error)?;
        if let Some((key, value)) = unknown_depth(&file) {
            let value = value.to_string();
            return Err(Error::UnknownDepth { key, value });
        }
        let raw: RawTable = toml::from_str(text).map_err(toml_error)?;
        let has_colors = raw.color.is_some();
        let colors = raw
            .color
            .unwrap_or_default()
            .into_iter()
            .map(|(name, rules)| Ok((name.parse()?, rules)))
            .collect::<Result<_, Error>>()?;
        if let Some((key, value)) = core_switched_off(&file) {
            let value = value.to_string();
            return Err(Error::CoreSwitchedOff { key, value });
        }
        let table = RulesTable {
            colors,
            acceptance: raw.acceptance,
        };
        Ok((table, has_colors))
    }

    /// The rule for a color in a lane: the lane's own table, else the color's table, else
    /// [`Rule::default`] when the table does not mention the color. Red always has the core of
    /// GDD on: a `false` or a quick review is refused when reading, and a key left out counts
    /// as on (`mutation` and `review`, whose defaults are off and quick, get `true` and deep).
    pub fn rule(&self, color: Color, lane: Lane) -> Rule {
        let rule = self.colors.get(&color).map_or_else(Rule::default, |c| {
            match lane {
                Lane::Fast => c.fast,
                Lane::Full => c.full,
            }
            .unwrap_or(c.rule)
        });
        if color == Color::Red {
            Rule {
                mutation: true,
                review: Depth::Deep,
                ..rule
            }
        } else {
            rule
        }
    }
}

/// The rules a color's table `rules` writes, each with its lane: the table without a lane
/// first, then lane `fast` and lane `full` when it has them.
fn lanes_of(rules: &toml::Value) -> impl Iterator<Item = (Option<Lane>, &toml::Value)> {
    let own = move |lane: Lane| {
        let rule = rules.get(lane.to_string().as_str())?;
        Some((Some(lane), rule))
    };
    let lanes = [Lane::Fast, Lane::Full].into_iter().filter_map(own);
    std::iter::once((None, rules)).chain(lanes)
}

/// The key `key` of the rule of `color` in `lane` as the rules file names it:
/// `color.<color>.<key>` without a lane, `color.<color>.<lane>.<key>` with one.
fn key_name(color: &str, lane: Option<Lane>, key: &str) -> String {
    match lane {
        None => format!("color.{color}.{key}"),
        Some(lane) => format!("color.{color}.{lane}.{key}"),
    }
}

/// The first review depth other than `"quick"` or `"deep"` the rules file `file` writes, with
/// its key, `color.<color>.review` or `color.<color>.<lane>.review`.
fn unknown_depth(file: &toml::Table) -> Option<(String, &toml::Value)> {
    let colors = file.get("color").and_then(toml::Value::as_table)?;
    colors.iter().find_map(|(color, rules)| {
        lanes_of(rules).find_map(|(lane, rule)| {
            let value = rule.get("review")?;
            let known = matches!(value.as_str(), Some("quick" | "deep"));
            (!known).then(|| (key_name(color, lane, "review"), value))
        })
    })
}

/// What a red change always gets, the core of GDD, as keys of a lane, in the order they are
/// reported: no rules file can switch them off for red (see [`switches_off`]).
const CORE: [&str; 4] = ["mutation", "reviewer", "g4_if_g1", "review"];

/// Whether `value`, written for the [`CORE`] key `key`, switches it off: `"quick"` for the
/// review, `false` for the rest.
fn switches_off(key: &str, value: &toml::Value) -> bool {
    match key {
        "review" => value.as_str() == Some("quick"),
        _ => value.as_bool() == Some(false),
    }
}

/// The first key of the [`CORE`] the rules file `file` switches off in a red table, with its
/// value, as `color.red.<key>` or `color.red.<lane>.<key>`: the table without a lane first,
/// then lane `fast` and lane `full`. A key left out does not switch anything off.
fn core_switched_off(file: &toml::Table) -> Option<(String, &toml::Value)> {
    let red = file.get("color")?.get("red")?;
    lanes_of(red).find_map(|(lane, rule)| {
        CORE.iter().find_map(|&key| {
            let value = rule.get(key).filter(|value| switches_off(key, value))?;
            Some((key_name("red", lane, key), value))
        })
    })
}

/// The table that ships with the tool; it equals the `[color.*]` and `[acceptance]` sections
/// of `gdd/rules.toml`, which leave the review unset: quick, and deep for red. Red has no fast
/// lane: it always travels the full one.
pub fn default_table() -> RulesTable {
    let rule = |tests_first, mutation, reviewer, g4_if_g1| Rule {
        tests_first: Some(tests_first),
        mutation,
        reviewer,
        g4_if_g1,
        ..Rule::default()
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
        review: Depth::Quick,
    };

    /// The same for red, which always has the core of GDD on.
    const RED_UNSET: Rule = Rule {
        mutation: true,
        review: Depth::Deep,
        ..UNSET
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
                review: Depth::Quick,
            }
        );
        assert_eq!(
            t.rule(Color::Red, Lane::Full),
            Rule {
                tests_first: Some(true),
                mutation: true,
                reviewer: true,
                g4_if_g1: true,
                review: Depth::Deep,
            }
        );
    }

    #[test]
    fn the_review_is_quick_unless_the_change_is_red() {
        let own = table("[color.yellow.full]\nmutation = true\n");
        for t in [default_table(), table(""), own] {
            for lane in [Lane::Fast, Lane::Full] {
                assert_eq!(t.rule(Color::Green, lane).review, Depth::Quick);
                assert_eq!(t.rule(Color::Yellow, lane).review, Depth::Quick);
                assert_eq!(t.rule(Color::Red, lane).review, Depth::Deep);
            }
        }
    }

    #[test]
    fn a_rules_file_can_ask_for_either_depth() {
        let text = "[color.green.fast]\nreview = \"deep\"\n[color.green.full]\nreview = \"quick\"\n\
                    [color.yellow]\nreview = \"deep\"\n[color.red.full]\nreview = \"deep\"\n";
        let t = table(text);
        assert_eq!(t.rule(Color::Green, Lane::Fast).review, Depth::Deep);
        assert_eq!(t.rule(Color::Green, Lane::Full).review, Depth::Quick);
        assert_eq!(t.rule(Color::Yellow, Lane::Fast).review, Depth::Deep);
        assert_eq!(t.rule(Color::Yellow, Lane::Full).review, Depth::Deep);
        assert_eq!(t.rule(Color::Red, Lane::Full), RED_UNSET);
        // A lane's own table replaces the color's, its depth too.
        let t = table("[color.yellow]\nreview = \"deep\"\n[color.yellow.full]\nmutation = true\n");
        assert_eq!(t.rule(Color::Yellow, Lane::Full).review, Depth::Quick);
    }

    /// The error of a rules file whose `key` sets the review depth to `value`, as TOML.
    fn depth_refused(key: &str, value: &str) -> Result<RulesTable, Error> {
        let (key, value) = (key.into(), value.into());
        Err(Error::UnknownDepth { key, value })
    }

    #[test]
    fn a_depth_that_is_neither_quick_nor_deep_is_refused_naming_its_key() {
        let yellow = ["color.yellow.fast", "color.yellow.full"];
        for table in ["color.green", "color.red"].into_iter().chain(yellow) {
            for value in ["\"medium\"", "\"Deep\"", "\"\"", "true", "1"] {
                let text = format!("[{table}]\nreview = {value}\n");
                let expected = depth_refused(&format!("{table}.review"), value);
                assert_eq!(RulesTable::from_toml(&text), expected, "{text}");
                assert_eq!(RulesTable::of_project(&text), expected, "{text}");
            }
        }
        let said = Error::UnknownDepth {
            key: "color.yellow.full.review".into(),
            value: "\"medium\"".into(),
        };
        let expected = "review must be quick or deep: color.yellow.full.review = \"medium\" in \
                        gdd/rules.toml";
        assert_eq!(said.to_string(), expected);
        // Only the rules the table reads are checked, and a valid depth beside a bad one passes.
        let ignored = "[color.yellow.slow]\nreview = \"x\"\n[color.yellow.full]\nnote = \"x\"\n";
        assert_eq!(table(ignored).rule(Color::Yellow, Lane::Full), UNSET);
        let both = "[color.green]\nreview = \"deep\"\n[color.yellow.fast]\nreview = \"x\"\n";
        let expected = depth_refused("color.yellow.fast.review", "\"x\"");
        assert_eq!(RulesTable::from_toml(both), expected);
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
        for lane in [Lane::Fast, Lane::Full] {
            assert_eq!(t.rule(Color::Green, lane), UNSET);
            assert_eq!(t.rule(Color::Yellow, lane), UNSET);
            assert_eq!(t.rule(Color::Red, lane), RED_UNSET);
        }
    }

    #[test]
    fn other_lanes_and_keys_are_ignored() {
        let t = table("[color.yellow.slow]\nmutation = true\n[color.yellow.full]\nnote = \"x\"\n");
        assert_eq!(t.rule(Color::Yellow, Lane::Full), UNSET);
        assert_eq!(t.rule(Color::Yellow, Lane::Fast), UNSET);
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

    /// The error of a rules file whose `key` switches off the core for red changes with
    /// `value`, as TOML.
    fn switched_off(key: &str, value: &str) -> Result<RulesTable, Error> {
        let (key, value) = (key.into(), value.into());
        Err(Error::CoreSwitchedOff { key, value })
    }

    /// The error of a rules file that sets `key` to `false` for red changes.
    fn core_off(key: &str) -> Result<RulesTable, Error> {
        switched_off(key, "false")
    }

    #[test]
    fn no_red_table_can_switch_off_the_core() {
        for table in ["color.red", "color.red.fast", "color.red.full"] {
            for key in ["mutation", "reviewer", "g4_if_g1"] {
                let text = format!("[{table}]\n{key} = false\n");
                let expected = core_off(&format!("{table}.{key}"));
                assert_eq!(RulesTable::from_toml(&text), expected, "{text}");
                assert_eq!(RulesTable::of_project(&text), expected, "{text}");
            }
            let text = format!("[{table}]\nreview = \"quick\"\n");
            let expected = switched_off(&format!("{table}.review"), "\"quick\"");
            assert_eq!(RulesTable::from_toml(&text), expected, "{text}");
            assert_eq!(RulesTable::of_project(&text), expected, "{text}");
        }
        let said = core_off("color.red.full.mutation").unwrap_err().to_string();
        let expected = "red changes always get mutation testing, the Reviewer and the human G4 \
                        (the core of GDD): remove color.red.full.mutation = false from \
                        gdd/rules.toml";
        assert_eq!(said, expected);
        let quick = switched_off("color.red.review", "\"quick\"");
        let said = quick.unwrap_err().to_string();
        let expected = "red changes always get mutation testing, the Reviewer and the human G4 \
                        (the core of GDD): remove color.red.review = \"quick\" from \
                        gdd/rules.toml";
        assert_eq!(said, expected);
    }

    #[test]
    fn the_first_key_switched_off_is_named_table_without_lane_first_then_by_key() {
        let read = RulesTable::from_toml;
        let all = "[color.red]\nreviewer = false\n[color.red.fast]\nmutation = false\n\
                   [color.red.full]\nmutation = false\n";
        assert_eq!(read(all), core_off("color.red.reviewer"));
        let both = "[color.red.full]\nmutation = false\n[color.red.fast]\ng4_if_g1 = false\n";
        assert_eq!(read(both), core_off("color.red.fast.g4_if_g1"));
        let keys = "[color.red.full]\ng4_if_g1 = false\nreviewer = false\nmutation = false\n";
        assert_eq!(read(keys), core_off("color.red.full.mutation"));
        let last = "[color.red.full]\ng4_if_g1 = false\nreviewer = false\nmutation = true\n";
        assert_eq!(read(last), core_off("color.red.full.reviewer"));
        // The review comes after g4_if_g1.
        let review = "[color.red.full]\nreview = \"quick\"\ng4_if_g1 = false\n";
        assert_eq!(read(review), core_off("color.red.full.g4_if_g1"));
        let lanes = "[color.red.full]\nmutation = false\n[color.red]\nreview = \"quick\"\n";
        let expected = switched_off("color.red.review", "\"quick\"");
        assert_eq!(read(lanes), expected);
    }

    #[test]
    fn a_red_table_can_ask_for_the_deep_review_it_gets() {
        for name in ["color.red", "color.red.fast", "color.red.full"] {
            let text = format!("[{name}]\nreview = \"deep\"\n");
            let t = table(&text);
            assert_eq!(t.rule(Color::Red, Lane::Fast), RED_UNSET, "{text}");
            assert_eq!(t.rule(Color::Red, Lane::Full), RED_UNSET, "{text}");
        }
        // Quick stays allowed outside red.
        let text = "[color.green]\nreview = \"quick\"\n[color.yellow.full]\nreview = \"quick\"\n";
        assert_eq!(table(text).rule(Color::Yellow, Lane::Full), UNSET);
    }

    #[test]
    fn a_red_table_that_leaves_the_core_out_gets_it() {
        let t = table("[color.red.full]\ntests_first = false\n");
        let expected = Rule {
            tests_first: Some(false),
            ..RED_UNSET
        };
        assert_eq!(t.rule(Color::Red, Lane::Full), expected);
        let t = table("[color.red.fast]\nmutation = true\n");
        assert_eq!(t.rule(Color::Red, Lane::Fast), RED_UNSET);
        // The table without a lane, and so both lanes, gets it too.
        let t = table("[color.red]\ntests_first = true\n");
        let expected = Rule {
            tests_first: Some(true),
            ..RED_UNSET
        };
        assert_eq!(t.rule(Color::Red, Lane::Fast), expected);
        assert_eq!(t.rule(Color::Red, Lane::Full), expected);
        let on = "[color.red.full]\nmutation = true\nreviewer = true\ng4_if_g1 = true\n";
        assert_eq!(table(on).rule(Color::Red, Lane::Full), RED_UNSET);
        // Other colors keep the defaults.
        let t = table("[color.yellow.full]\ntests_first = false\n");
        let expected = Rule {
            tests_first: Some(false),
            ..UNSET
        };
        assert_eq!(t.rule(Color::Yellow, Lane::Full), expected);
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
    fn a_spike_has_the_budget_of_the_rules_file_or_five_usd() {
        assert_eq!(spike_budget(""), Ok(5.0));
        assert_eq!(SPIKE_USD, 5.0);
        let other = "[limits]\nreplans = 1\n[spike]\nnote = \"x\"\n";
        assert_eq!(spike_budget(other), Ok(5.0));
        assert_eq!(spike_budget("[spike]\nmax_usd = 0.02\n"), Ok(0.02));
        assert_eq!(spike_budget("[spike]\nmax_usd = 7\n"), Ok(7.0));
        let tiny = "[spike]\nmax_usd = 1e-300\n";
        assert_eq!(spike_budget(tiny), Ok(1e-300));
    }

    #[test]
    fn a_spike_budget_that_is_not_a_positive_number_names_its_key() {
        for (value, shown) in [
            ("0", "0"),
            ("0.0", "0.0"),
            ("-1", "-1"),
            ("-0.5", "-0.5"),
            ("\"5\"", "\"5\""),
            ("true", "true"),
            ("nan", "nan"),
            ("inf", "inf"),
            ("[5]", "[5]"),
        ] {
            let text = format!("[spike]\nmax_usd = {value}\n");
            let said = format!("spike.max_usd must be a positive number of USD, not {shown}");
            assert_eq!(spike_budget(&text), Err(Error::Toml(said)), "{value}");
        }
        let said = spike_budget("[spike]\nmax_usd = 0\n").unwrap_err();
        assert_eq!(
            said.to_string(),
            "invalid TOML: spike.max_usd must be a positive number of USD, not 0"
        );
        for wrong in ["spike = 3\n", "[spike", "spike.max_usd = "] {
            let result = spike_budget(wrong);
            assert!(matches!(result, Err(Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn the_commands_have_the_time_limit_of_the_rules_file_or_thirty_minutes() {
        assert_eq!(command_seconds(""), Ok(Duration::from_secs(1800)));
        assert_eq!(exec::TIMEOUT, Duration::from_secs(1800));
        let other = "[limits]\nreplans = 1\n[spike]\nmax_usd = 5\n";
        assert_eq!(command_seconds(other), Ok(exec::TIMEOUT));
        let one = "[limits]\ncommand_seconds = 1\n";
        assert_eq!(command_seconds(one), Ok(Duration::from_secs(1)));
        let long =
            "main_branch = \"main\"\n[limits]\nbuilder_retries = 3\ncommand_seconds = 7200\n";
        assert_eq!(command_seconds(long), Ok(Duration::from_secs(7200)));
    }

    #[test]
    fn a_time_limit_that_is_not_a_whole_number_above_zero_names_its_key() {
        for (value, shown) in [
            ("0", "0"),
            ("-1", "-1"),
            ("-1800", "-1800"),
            ("1.5", "1.5"),
            ("60.0", "60.0"),
            ("\"60\"", "\"60\""),
            ("\"\"", "\"\""),
            ("true", "true"),
            ("[60]", "[60]"),
        ] {
            let text = format!("[limits]\ncommand_seconds = {value}\n");
            let said = format!(
                "limits.command_seconds must be a whole number of seconds above 0, not {shown}"
            );
            assert_eq!(command_seconds(&text), Err(Error::Toml(said)), "{value}");
        }
        let said = command_seconds("[limits]\ncommand_seconds = 0\n").unwrap_err();
        assert_eq!(
            said.to_string(),
            "invalid TOML: limits.command_seconds must be a whole number of seconds above 0, not 0"
        );
        for wrong in ["limits = 3\n", "[limits", "limits.command_seconds = "] {
            let result = command_seconds(wrong);
            assert!(matches!(result, Err(Error::Toml(_))), "{wrong:?}");
        }
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
