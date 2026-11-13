//! The deterministic layer of Gate-Driven Development (GDD), contract `gdd/1`.
//!
//! The library reads the files the GDD skills leave in a project (the blueprint, the logs, the
//! briefs, the backlog) and keeps the order, the accounting and the evidence of each change. The
//! pure parts (reading the blueprint, deriving a change's state from its events, counting limits,
//! computing metrics) take data and return data. Git and process launching sit behind the small
//! functions of [`git`] and [`launch`].

pub mod backlog;
pub mod blueprint;
pub mod color;
pub mod commands;
pub mod gates;
pub mod git;
pub mod journal;
pub mod launch;
pub mod limits;
pub mod log;
pub mod metrics;
pub mod out;
pub mod project;
pub mod report;
pub mod runs;
pub mod seal;
pub mod ship;
pub mod state;
pub mod time;
pub mod verify;

pub use out::{Fail, Out};

/// The version of the contract this layer reads and writes.
pub const CONTRACT: &str = "gdd/1";

/// The three colors of the method, from the lightest.
pub const COLORS: [&str; 3] = ["green", "yellow", "red"];

/// Where a color stands among the three, or `None` for a word that is not a color.
pub fn color_rank(color: &str) -> Option<usize> {
    COLORS.iter().position(|c| *c == color)
}

/// The higher of two colors. An unknown word never wins over a color.
pub fn higher<'a>(a: &'a str, b: &'a str) -> &'a str {
    match (color_rank(a), color_rank(b)) {
        (Some(x), Some(y)) => {
            if y > x {
                b
            } else {
                a
            }
        }
        (None, Some(_)) => b,
        _ => a,
    }
}

/// An id of the contract: lowercase letters, digits and hyphens.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
