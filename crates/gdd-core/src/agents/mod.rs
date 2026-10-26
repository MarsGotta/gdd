//! The coding agents that carry a change: which model plays each role ([`models`]) and how many
//! rounds the Builder gets against G2.

pub mod models;

use serde::Deserialize;

use crate::config;

/// Builder rounds against G2 when the rules file says nothing.
const RETRIES: u32 = 3;

/// The Builder rounds against G2 before a change is parked: `builder_retries` in the `[limits]`
/// section of a rules file; 3 when it says nothing. The rest of the file is ignored.
pub fn builder_retries(rules: &str) -> Result<u32, config::Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Raw {
        limits: Limits,
    }
    #[derive(Deserialize)]
    #[serde(default)]
    struct Limits {
        builder_retries: u32,
    }
    impl Default for Limits {
        fn default() -> Limits {
            Limits {
                builder_retries: RETRIES,
            }
        }
    }
    let raw: Raw = toml::from_str(rules).map_err(|e| config::Error::Toml(e.to_string()))?;
    Ok(raw.limits.builder_retries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builder_gets_the_retries_of_the_limits_or_three() {
        assert_eq!(builder_retries(""), Ok(3));
        assert_eq!(builder_retries("[limits]\nreplans = 1\n"), Ok(3));
        let rules = "main_branch = \"main\"\n[limits]\nbuilder_retries = 5\n";
        assert_eq!(builder_retries(rules), Ok(5));
        assert_eq!(builder_retries("[limits]\nbuilder_retries = 0\n"), Ok(0));
        for wrong in [
            "[limits]\nbuilder_retries = -1\n",
            "[limits]\nbuilder_retries = \"3\"\n",
            "limits = 3\n",
            "[limits",
        ] {
            let result = builder_retries(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }
}
