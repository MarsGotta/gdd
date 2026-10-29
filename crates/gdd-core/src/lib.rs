//! The GDD (Gate-Driven Development) method as a library.
//!
//! Work moves forward by passing gates; risk decides how many gates and how strict.
//! This crate holds the method itself and has no terminal or CLI dependencies.

pub mod adopt;
pub mod agents;
pub mod change;
pub mod changelog;
pub mod classify;
pub mod config;
pub mod delivery;
pub mod exec;
pub mod flow;
pub mod gates;
pub mod git;
pub mod guard;
pub mod inbox;
pub mod init;
pub mod learn;
pub mod log;
pub mod recipe;
pub mod repo;
pub mod report;
pub mod rules;
pub mod session;
pub mod stacks;
pub mod state;
pub mod status;
pub mod versions;
pub mod zones;

pub use classify::{Change, Classification, classify};
pub use config::{Color, Error, Lane};
pub use report::Report;
pub use rules::RulesTable;
pub use zones::ZoneMap;

/// Version of the `gdd` tool.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One line that identifies the tool, used by `gdd --version`.
pub fn version_line() -> String {
    format!("gdd {VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_line_names_the_tool() {
        assert_eq!(version_line(), format!("gdd {VERSION}"));
        assert!(version_line().starts_with("gdd "));
    }
}
