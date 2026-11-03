//! What the tool does with the user's repository beyond reading it (see [`crate::git`]): closing
//! a change into the main branch ([`merge`]), bringing it up to date with the main branch
//! ([`integrate`]), delivering it once it joined the main branch, from the main checkout
//! ([`deliver`]), and the blueprint of software the core asks where a change works, what it
//! touched, what it keeps and how it stands against the main branch ([`touched`]).

pub mod deliver;
pub mod integrate;
pub mod merge;
pub mod touched;
