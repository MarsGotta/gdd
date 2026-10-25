# ADR 0001 · Technology

- Status: accepted (Setup)
- Date: 2026-10-24

## Context

`gdd` is a command line tool with a terminal UI that launches coding agents, runs gate commands and keeps state on disk. It will be published as open source and must be easy to install. The bootstrap kit is Python on top of Spec Kit, which needs Python and uv and brought three integration problems (commands registered only for the active agent, broken dev-mode links, text-only capture that hid token usage).

## Decision

Rust, as a Cargo workspace:

- `crates/gdd-core`: library with the method (config, zones and color, rules table, gates, log, runs). No terminal or CLI dependencies.
- `crates/gdd`: the binary (CLI with clap, TUI with ratatui + crossterm).
- `crates/gdd-acceptance`: Gherkin scenarios run with cucumber-rs, talking to `gdd-core` (not to the screen).

Toolchain pinned in `rust-toolchain.toml`. Gate tools: rustfmt, clippy, cargo test, cucumber-rs, cargo-deny (advisories, licenses, sources), cargo-mutants (mutation testing on the diff).

## Alternatives

- Python + Textual, like Jezero: fastest to write (the logic already exists in Python), but needs a Python runtime to install.
- Go + Bubble Tea: single binary too, but no existing experience in the author's projects, while Orbit is already Rust.

## Consequences

- One binary to install; good fit for open source distribution.
- The Python kit is the specification: its behavior and its end-to-end cases are ported, not redesigned.
- Rust compile times make G2 slower than the Python kit; acceptance scenarios must stay at the core level to keep G2 fast.
