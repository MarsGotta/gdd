# gdd

**GDD · Gate-Driven Development**: build software with AI coding agents by passing gates. Risk decides how many gates and how strict; machines guard what can be checked, and the human decides only what needs a human.

`gdd` will be a single Rust binary with two faces:

- a **CLI** for agents, scripts and CI;
- a **TUI** for the human: changes by phase, a sitting inbox for approvals, a live view of running changes and token reports.

> Status: **pre-alpha, under construction.** Nothing here is usable yet.

## Built with itself

`gdd` is being built with GDD. Until the Rust binary can run its own gates, the harness is the Python bootstrap kit in `gdd-kit/` (on top of [Spec Kit](https://github.com/github/spec-kit)). The final tool does not depend on Spec Kit.

- Method: [`GDD.md`](GDD.md)
- Project Brief: [`docs/brief.md`](docs/brief.md)
- Roadmap (what 0.1 will have, and its status): [`ROADMAP.md`](ROADMAP.md)
- Decisions: [`docs/adr/`](docs/adr/)
- Scenarios: [`features/`](features/)

## Development

Requires Rust (the toolchain is pinned in `rust-toolchain.toml`), Python 3.12+ for the bootstrap kit, and `cargo-deny` and `cargo-mutants` for the G3 gate.

```sh
cargo test --workspace           # unit tests and Gherkin scenarios
bin/gdd status                   # where the project stands in GDD
bin/gdd selftest                 # harness self-test (4/4)
gdd-kit/install.sh               # (re)install the bootstrap kit for claude, codex and opencode
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
