//! The software blueprint: the gates software declares, as data the core reads (see
//! [`crate::gates::decl`]). `works` checks that the change does what its scenarios say, `quality`
//! that the code stays clean and `security` that nothing unsafe gets in; each claims the kinds of
//! command of ADR 0003 section 6, and `works` takes the commands no gate claims.

use crate::config;
use crate::gates::decl::Declared;

/// The declaration of the software blueprint's gates, built into the tool.
pub const GATES: &str = include_str!("../../assets/blueprints/software.toml");

/// The software blueprint's gates, as the core reads them (see [`Declared::from_toml`]).
pub fn gates() -> Result<Declared, config::Error> {
    Declared::from_toml(GATES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::Gate;

    /// The ids of the gates, in the order the blueprint declares them.
    fn ids(declared: &Declared) -> Vec<&str> {
        let gates = declared.gates().iter();
        gates.map(|gate| gate.id.as_str()).collect()
    }

    #[test]
    fn the_shipped_gates_read_in_their_order() {
        let declared = gates().unwrap();
        assert_eq!(ids(&declared), ["works", "quality", "security"]);
        assert_eq!(Declared::from_toml(GATES), Ok(declared));
    }

    #[test]
    fn each_command_goes_to_the_gate_of_its_kind_and_works_takes_the_rest() {
        let declared = gates().unwrap();
        for (command, gate) in [
            ("unit", "works"),
            ("acceptance", "works"),
            ("smoke", "works"),
            ("acceptance_full", "works"),
            ("lint", "quality"),
            ("types", "quality"),
            ("architecture", "quality"),
            ("architecture-frontier", "quality"),
            ("quality", "quality"),
            ("mutation", "quality"),
            ("mutation-slow", "quality"),
            ("mutation_slow", "quality"),
            ("security", "security"),
            ("dependencies", "security"),
            ("lockfile", "security"),
            ("secrets", "security"),
        ] {
            assert_eq!(declared.claim(command).id.as_str(), gate, "{command}");
        }
    }

    #[test]
    fn every_mutation_command_is_optional_and_mutation_switches_it_on() {
        let declared = gates().unwrap();
        for command in ["mutation", "mutation-slow", "mutation_slow", "mutationx"] {
            assert_eq!(declared.optional(command), Some("mutation"), "{command}");
        }
        for command in ["lint", "unit", "secrets", "mutatio"] {
            assert_eq!(declared.optional(command), None, "{command}");
        }
    }

    #[test]
    fn the_scenario_count_is_works_and_the_exceptions_are_security_before_integrating() {
        let declared = gates().unwrap();
        let named = |stage, check| declared.named(stage, check).map(|g| g.id.as_str());
        assert_eq!(named(Gate::G3, "scenarios"), Some("works"));
        assert_eq!(named(Gate::G3, "exceptions"), Some("security"));
        assert_eq!(named(Gate::G2, "scenarios"), None);
        assert_eq!(named(Gate::G2, "exceptions"), None);
    }
}
