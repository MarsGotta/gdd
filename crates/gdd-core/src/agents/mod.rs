//! The coding agents that carry a change: which model plays each role ([`models`]), what each
//! agent is like ([`sheets`]), the GDD commands and the instructions of the project's stacks in
//! their folders ([`render`]), how the recipe starts them ([`invoke`]) with which environment
//! ([`env`]), what they say they used ([`usage`]), the files of GDD state they could have turned
//! into links ([`files`]), and the limits the rules file sets on them.

pub mod env;
pub mod files;
pub mod invoke;
pub mod models;
pub mod render;
pub mod sheets;
pub mod usage;

use serde::Deserialize;

use crate::config;

/// What the `[limits]` section of a rules file says about the agents. What it leaves out takes
/// the tool's value; the rest of the file is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Limits {
    /// Builder rounds against G2 before the replan: `builder_retries`, 3.
    pub builder_retries: u32,
    /// Replans of a change still red in G2 before it parks: `replans`, 1.
    pub replans: u32,
    /// Fix rounds after a red G3, and after a Reviewer KO, before the change parks:
    /// `reviewer_fixes`, 1, as before the rules could set it.
    pub reviewer_fixes: u32,
    /// Seconds an agent has to answer: `agent_timeout_s`, 3600.
    pub agent_timeout_s: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            builder_retries: 3,
            replans: 1,
            reviewer_fixes: 1,
            agent_timeout_s: 3600,
        }
    }
}

impl Limits {
    /// Reads the `[limits]` section of a rules file.
    pub fn from_toml(rules: &str) -> Result<Limits, config::Error> {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Raw {
            limits: Limits,
        }
        let raw: Raw = toml::from_str(rules).map_err(|e| config::Error::Toml(e.to_string()))?;
        Ok(raw.limits)
    }

    /// The limits `self` sets above those `before` sets, each with its key and both values,
    /// `before`'s first: a higher limit gives the agents more tries, so it loosens the gate.
    pub fn raised_from(&self, before: &Limits) -> Vec<(&'static str, u64, u64)> {
        let wide = u64::from;
        [
            (
                "builder_retries",
                wide(before.builder_retries),
                wide(self.builder_retries),
            ),
            ("replans", wide(before.replans), wide(self.replans)),
            (
                "reviewer_fixes",
                wide(before.reviewer_fixes),
                wide(self.reviewer_fixes),
            ),
            (
                "agent_timeout_s",
                before.agent_timeout_s,
                self.agent_timeout_s,
            ),
        ]
        .into_iter()
        .filter(|&(_, old, new)| new > old)
        .collect()
    }

    /// Each limit of `self` and `other`, the lower of the two: what is left of both when neither
    /// may loosen the gate.
    pub fn lowest(&self, other: &Limits) -> Limits {
        Limits {
            builder_retries: self.builder_retries.min(other.builder_retries),
            replans: self.replans.min(other.replans),
            reviewer_fixes: self.reviewer_fixes.min(other.reviewer_fixes),
            agent_timeout_s: self.agent_timeout_s.min(other.agent_timeout_s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limits_are_the_rules_or_the_tools() {
        let tool = Limits {
            builder_retries: 3,
            replans: 1,
            reviewer_fixes: 1,
            agent_timeout_s: 3600,
        };
        assert_eq!(Limits::from_toml(""), Ok(tool));
        assert_eq!(Limits::default(), tool);
        let other = "[limits]\nreviewer_fixes = 1\n";
        assert_eq!(Limits::from_toml(other), Ok(tool));
        let two = Limits::from_toml("[limits]\nreviewer_fixes = 2\n").unwrap();
        assert_eq!(two.reviewer_fixes, 2);
        let none = Limits::from_toml("[limits]\nreviewer_fixes = 0\n").unwrap();
        assert_eq!(none.reviewer_fixes, 0);
        let rules = "main_branch = \"main\"\n[limits]\nbuilder_retries = 5\nreplans = 0\n\
                     agent_timeout_s = 2\n";
        let set = Limits {
            builder_retries: 5,
            replans: 0,
            reviewer_fixes: 1,
            agent_timeout_s: 2,
        };
        assert_eq!(Limits::from_toml(rules), Ok(set));
        let one = Limits::from_toml("[limits]\nreplans = 2\n").unwrap();
        assert_eq!((one.builder_retries, one.replans), (3, 2));
        for wrong in [
            "[limits]\nbuilder_retries = -1\n",
            "[limits]\nbuilder_retries = \"3\"\n",
            "[limits]\nagent_timeout_s = 1.5\n",
            "[limits]\nreviewer_fixes = -1\n",
            "[limits]\nreviewer_fixes = \"2\"\n",
            "[limits]\nreviewer_fixes = 1.5\n",
            "limits = 3\n",
            "[limits",
        ] {
            let result = Limits::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn the_limits_raised_are_the_higher_ones_with_both_values() {
        let before = Limits::default();
        assert_eq!(before.raised_from(&before), Vec::new());
        let after = Limits {
            builder_retries: 5,
            replans: 0,
            reviewer_fixes: 2,
            agent_timeout_s: 3600,
        };
        assert_eq!(
            after.raised_from(&before),
            vec![("builder_retries", 3, 5), ("reviewer_fixes", 1, 2)]
        );
        assert_eq!(before.raised_from(&after), vec![("replans", 0, 1)]);
        let longer = Limits {
            agent_timeout_s: 7200,
            ..before
        };
        assert_eq!(
            longer.raised_from(&before),
            vec![("agent_timeout_s", 3600, 7200)]
        );
    }

    #[test]
    fn the_lowest_limits_take_each_limit_from_the_one_that_sets_it_lower() {
        let one = Limits {
            builder_retries: 5,
            replans: 0,
            reviewer_fixes: 2,
            agent_timeout_s: 60,
        };
        let other = Limits {
            builder_retries: 2,
            replans: 4,
            reviewer_fixes: 0,
            agent_timeout_s: 7200,
        };
        let lowest = Limits {
            builder_retries: 2,
            replans: 0,
            reviewer_fixes: 0,
            agent_timeout_s: 60,
        };
        assert_eq!(one.lowest(&other), lowest);
        assert_eq!(other.lowest(&one), lowest);
        assert_eq!(one.lowest(&one), one);
    }
}
