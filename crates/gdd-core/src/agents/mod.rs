//! The coding agents that carry a change: which model plays each role ([`models`]), what each
//! agent is like ([`sheets`]), the GDD commands in their folders ([`render`]), how the recipe
//! starts them ([`invoke`]) with which environment ([`env`]), what they say they used
//! ([`usage`]), the files of GDD state they could have turned into links ([`files`]), and the
//! limits the rules file sets on them.

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
    /// Seconds an agent has to answer: `agent_timeout_s`, 3600.
    pub agent_timeout_s: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            builder_retries: 3,
            replans: 1,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limits_are_the_rules_or_the_tools() {
        let tool = Limits {
            builder_retries: 3,
            replans: 1,
            agent_timeout_s: 3600,
        };
        assert_eq!(Limits::from_toml(""), Ok(tool));
        let other = "[limits]\nreviewer_fixes = 1\n";
        assert_eq!(Limits::from_toml(other), Ok(tool));
        let rules = "main_branch = \"main\"\n[limits]\nbuilder_retries = 5\nreplans = 0\n\
                     agent_timeout_s = 2\n";
        let set = Limits {
            builder_retries: 5,
            replans: 0,
            agent_timeout_s: 2,
        };
        assert_eq!(Limits::from_toml(rules), Ok(set));
        let one = Limits::from_toml("[limits]\nreplans = 2\n").unwrap();
        assert_eq!((one.builder_retries, one.replans), (3, 2));
        for wrong in [
            "[limits]\nbuilder_retries = -1\n",
            "[limits]\nbuilder_retries = \"3\"\n",
            "[limits]\nagent_timeout_s = 1.5\n",
            "limits = 3\n",
            "[limits",
        ] {
            let result = Limits::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }
}
