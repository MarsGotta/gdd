//! The rules table: which gates each color and lane asks for.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::config::{Color, Error, Lane};

/// What one color in one lane asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Rule {
    /// Tests are written before the code.
    pub tests_first: bool,
    /// Mutation testing runs.
    pub mutation: bool,
    /// The Reviewer checks the change.
    pub reviewer: bool,
    /// New scenarios also need the human release gate (G4).
    pub g4_if_g1: bool,
}

/// The rules for every color and lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesTable {
    rules: BTreeMap<(Color, Lane), Rule>,
}

#[derive(Deserialize)]
struct RawTable {
    #[serde(default)]
    color: BTreeMap<String, BTreeMap<String, Rule>>,
}

impl RulesTable {
    /// Reads the `[color.<color>.<lane>]` sections of a rules file; the rest is ignored.
    pub fn from_toml(text: &str) -> Result<RulesTable, Error> {
        let raw: RawTable = toml::from_str(text).map_err(|e| Error::Toml(e.to_string()))?;
        let mut rules = BTreeMap::new();
        for (color, lanes) in raw.color {
            let color: Color = color.parse()?;
            for (lane, rule) in lanes {
                rules.insert((color, lane.parse()?), rule);
            }
        }
        Ok(RulesTable { rules })
    }

    /// The rule for a color in a lane.
    pub fn rule(&self, color: Color, lane: Lane) -> Result<Rule, Error> {
        self.rules
            .get(&(color, lane))
            .copied()
            .ok_or(Error::MissingRule(color, lane))
    }
}

/// The table that ships with the tool; it equals the `[color.*]` sections of `gdd/rules.toml`.
/// Red has no fast lane: it always travels the full one.
pub fn default_table() -> RulesTable {
    let rule = |tests_first, mutation, reviewer, g4_if_g1| Rule {
        tests_first,
        mutation,
        reviewer,
        g4_if_g1,
    };
    RulesTable {
        rules: BTreeMap::from([
            ((Color::Green, Lane::Fast), rule(false, false, false, false)),
            ((Color::Green, Lane::Full), rule(true, false, true, true)),
            ((Color::Yellow, Lane::Fast), rule(true, true, true, false)),
            ((Color::Yellow, Lane::Full), rule(true, true, true, true)),
            ((Color::Red, Lane::Full), rule(true, true, true, true)),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_table_equals_the_rules_file() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../gdd/rules.toml");
        let text = std::fs::read_to_string(path).expect("gdd/rules.toml");
        assert_eq!(RulesTable::from_toml(&text), Ok(default_table()));
    }

    #[test]
    fn red_has_no_fast_lane() {
        assert_eq!(
            default_table().rule(Color::Red, Lane::Fast),
            Err(Error::MissingRule(Color::Red, Lane::Fast))
        );
    }

    #[test]
    fn unknown_names_in_the_table_are_errors() {
        let rule = "tests_first = true\nmutation = true\nreviewer = true\ng4_if_g1 = true\n";
        assert_eq!(
            RulesTable::from_toml(&format!("[color.purple.full]\n{rule}")),
            Err(Error::UnknownColor("purple".into()))
        );
        assert_eq!(
            RulesTable::from_toml(&format!("[color.red.slow]\n{rule}")),
            Err(Error::UnknownLane("slow".into()))
        );
    }
}
