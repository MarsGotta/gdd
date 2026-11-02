//! What the tool does with the user's repository beyond reading it (see [`crate::git`]): closing
//! a change into the main branch ([`merge`]), and the blueprint of software the core asks where a
//! change works, what it touched and what it keeps ([`touched`]).

pub mod merge;
pub mod touched;
