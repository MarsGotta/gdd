//! Basic vocabulary of the method: colors, lanes and the errors of reading configuration.

use std::fmt;
use std::str::FromStr;

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

impl FromStr for Lane {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "fast" => Ok(Lane::Fast),
            "full" => Ok(Lane::Full),
            other => Err(Error::UnknownLane(other.to_string())),
        }
    }
}

/// Everything that can go wrong when reading the zone map or the rules table, or classifying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The text is not valid TOML or does not have the expected shape.
    Toml(String),
    /// A color other than green, yellow or red.
    UnknownColor(String),
    /// A lane other than fast or full.
    UnknownLane(String),
    /// The rules table has no entry for this color and lane.
    MissingRule(Color, Lane),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Toml(message) => write!(f, "invalid TOML: {message}"),
            Error::UnknownColor(name) => {
                write!(f, "unknown color \"{name}\" (use green, yellow or red)")
            }
            Error::UnknownLane(name) => write!(f, "unknown lane \"{name}\" (use fast or full)"),
            Error::MissingRule(color, lane) => {
                write!(
                    f,
                    "the rules table has no rule for {color} in the {lane} lane"
                )
            }
        }
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
    fn colors_and_lanes_round_trip_through_text() {
        for c in [Color::Green, Color::Yellow, Color::Red] {
            assert_eq!(c.to_string().parse::<Color>(), Ok(c));
        }
        for l in [Lane::Fast, Lane::Full] {
            assert_eq!(l.to_string().parse::<Lane>(), Ok(l));
        }
    }

    #[test]
    fn unknown_color_error_names_it() {
        let err = "purple".parse::<Color>().unwrap_err();
        assert!(err.to_string().contains("purple"));
    }
}
