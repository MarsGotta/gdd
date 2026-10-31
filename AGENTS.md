# Instructions for agents

This repository builds `gdd`, the GDD tool, in Rust, and works with **GDD · Gate-Driven Development** itself (`GDD.md`). The harness is `gdd` itself: the `gdd` on the PATH is the binary built from `main`.

- Your role comes from the command that invokes you (`gdd-build`, `gdd-review`...: `/gdd-build` in Claude Code and OpenCode, `$gdd-build` in Codex). Stick to it and to its context.
- If nobody gave you a role, you are the **Orchestrator** talking with the human: start with the `gdd-status` command.
- Never change `gdd/`, `bin/`, `.github/` or approved scenarios unless your command says so.
- Talk to the human in the language set in `gdd/project.toml` (`language`).

## Build and check

```bash
cargo build --locked
cargo test --workspace --locked                                       # unit tests and every scenario
GDD_FEATURES=features/<name>.feature cargo test -q -p gdd-acceptance  # one feature; add GDD_HIDDEN=1 for its hidden examples
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all --check
```

- A change is done when `gdd g2 <change>` is green; G3 (`gdd g3 <change>`) adds every scenario, mutation testing, advisories, licenses, the lock file and secrets. The commands are in `gdd/commands.toml`.
- `features/` is the executable specification (cucumber-rs, strict: an undefined step fails). Steps live in `crates/gdd-acceptance/tests/acceptance.rs`.

## Files you do not edit by hand

- `.claude/`, `.agents/`, `.opencode/`: written by `gdd agents install` from `crates/gdd-core/assets/commands/*.md`. Change the asset.
- `docs/reference/cli/` and `docs/es/reference/cli/`: written by `python3 scripts/make_reference.py` from `gdd --help` and the notes in `scripts/cli_notes/<command>.md`. Change the help text in the code or the note, then run it (`--check` tells whether a page is out of date).
- `CHANGELOG.md` lines: written by `gdd changelog --write` from the merge commits.

## Documentation

- `docs/` is Markdown read first on GitHub, organised by Diátaxis (`getting-started/`, `how-to/`, `reference/`, `explanation/`); the rules are in `docs/STYLE.md`.
- Every English page has a Spanish twin in `docs/es/` with the same path and headings. Change both.
- Before you finish: `python3 scripts/check_md_links.py .` and `python3 scripts/check_translation.py` report 0 problems. No em or en dashes; this repository is public, so no private names, hosts or paths.
- Output blocks are real output of the current `gdd`, never written by hand.
- `python3 scripts/build_site.py` builds the site (Zensical), with an `llms.txt` and a Markdown copy of every page for agents.
