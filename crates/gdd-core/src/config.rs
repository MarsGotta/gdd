//! Basic vocabulary of the method: colors, lanes and the errors of reading configuration.

use std::fmt;
use std::str::FromStr;

use crate::guard::RULES;
use crate::layout::Layout;

/// Risk color of a change. The order is the order of risk: green < yellow < red.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Color {
    /// Low risk.
    Green,
    /// Normal risk.
    Yellow,
    /// Gate integrity: the highest risk.
    Red,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Color::Green => "green",
            Color::Yellow => "yellow",
            Color::Red => "red",
        })
    }
}

impl FromStr for Color {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "green" => Ok(Color::Green),
            "yellow" => Ok(Color::Yellow),
            "red" => Ok(Color::Red),
            other => Err(Error::UnknownColor(other.to_string())),
        }
    }
}

/// Lane a change travels: `fast` skips gates that `full` requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lane {
    /// Reduced set of gates.
    Fast,
    /// Every gate its color asks for.
    Full,
}

impl fmt::Display for Lane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Lane::Fast => "fast",
            Lane::Full => "full",
        })
    }
}

/// Everything that can go wrong when reading the zone map or the rules table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The text is not valid TOML or does not have the expected shape.
    Toml(String),
    /// A color other than green, yellow or red.
    UnknownColor(String),
    /// The rules table switches off part of the core of GDD for red changes: the key that does
    /// it, `color.red.<key>` or `color.red.<lane>.<key>`, and its value as TOML (`false` for a
    /// gate, `"quick"` for the review).
    CoreSwitchedOff {
        /// The key that switches it off.
        key: String,
        /// Its value as TOML.
        value: String,
    },
    /// A review depth other than `quick` or `deep`: the key that sets it,
    /// `color.<color>.review` or `color.<color>.<lane>.review`, and its value as TOML.
    UnknownDepth {
        /// The key that sets it.
        key: String,
        /// Its value as TOML.
        value: String,
    },
}

impl Error {
    /// The error as a project of `layout` reads it: the rules file that a red table switching
    /// off the core, or a review depth that is neither, names is where that project keeps it
    /// (see [`Layout::path`]). The other errors name no file.
    pub fn in_layout(&self, layout: Layout) -> String {
        Said(self, layout).to_string()
    }
}

/// An error as a project of a layout reads it (see [`Error::in_layout`]).
struct Said<'a>(&'a Error, Layout);

impl fmt::Display for Said<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Said(error, layout) = *self;
        match error {
            Error::Toml(message) => write!(f, "invalid TOML: {message}"),
            Error::UnknownColor(name) => {
                write!(f, "unknown color \"{name}\" (use green, yellow or red)")
            }
            Error::CoreSwitchedOff { key, value } => write!(
                f,
                "red changes always get mutation testing, the Reviewer and the human G4 \
                 (the core of GDD): remove {key} = {value} from {}",
                layout.path(RULES)
            ),
            Error::UnknownDepth { key, value } => write!(
                f,
                "review must be quick or deep: {key} = {value} in {}",
                layout.path(RULES)
            ),
        }
    }
}

impl fmt::Display for Error {
    /// The error as a project of the new layout reads it (see [`Error::in_layout`]).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Said(self, Layout::Tminus))
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_ordered_by_risk() {
        assert!(Color::Green < Color::Yellow);
        assert!(Color::Yellow < Color::Red);
    }

    #[test]
    fn colors_round_trip_through_text() {
        for c in [Color::Green, Color::Yellow, Color::Red] {
            assert_eq!(c.to_string().parse::<Color>(), Ok(c));
        }
    }

    #[test]
    fn lanes_print_their_name() {
        assert_eq!(Lane::Fast.to_string(), "fast");
        assert_eq!(Lane::Full.to_string(), "full");
    }

    #[test]
    fn unknown_color_error_names_it() {
        let err = "purple".parse::<Color>().unwrap_err();
        assert!(err.to_string().contains("purple"));
    }

    #[test]
    fn the_rules_file_an_error_names_is_the_one_of_the_project_layout() {
        let core = Error::CoreSwitchedOff {
            key: "color.red.full.mutation".into(),
            value: "false".into(),
        };
        let depth = Error::UnknownDepth {
            key: "color.yellow.review".into(),
            value: "\"medium\"".into(),
        };
        let remove = "the human G4 (the core of GDD): remove color.red.full.mutation = false from";
        let unknown = "review must be quick or deep: color.yellow.review = \"medium\" in";
        for (layout, file) in [(Layout::Tminus, "tminus"), (Layout::Gdd, "gdd")] {
            let rules = format!("{file}/rules.toml");
            let said = core.in_layout(layout);
            assert!(said.ends_with(&format!("{remove} {rules}")), "{said}");
            assert_eq!(depth.in_layout(layout), format!("{unknown} {rules}"));
        }
        assert_eq!(core.to_string(), core.in_layout(Layout::Tminus));
        assert_eq!(depth.to_string(), depth.in_layout(Layout::Tminus));
        // The others name no file.
        let toml = Error::Toml("x".into());
        assert_eq!(toml.in_layout(Layout::Gdd), "invalid TOML: x");
        let color = Error::UnknownColor("tminus".into());
        let said = "unknown color \"tminus\" (use green, yellow or red)";
        assert_eq!(color.in_layout(Layout::Gdd), said);
    }
}
