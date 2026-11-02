//! What the tool does with the user's repository beyond reading it (see [`crate::git`]): closing
//! a change into the main branch ([`merge`]), and telling the core what a change touched, as
//! software keeps it ([`touched`]).

pub mod merge;
pub mod touched;
