//! The program `tminus`: the tool that runs GDD, Gate-Driven Development.

use std::process::ExitCode;

fn main() -> ExitCode {
    gdd::entry()
}
