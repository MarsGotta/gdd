# Project Brief · gdd

## What it is

`gdd` is the tool for **GDD · Gate-Driven Development**: a way of building software with AI agents where work moves forward by passing gates, and risk decides how many gates and how strict. One Rust binary with two faces:

- a **CLI** for agents, scripts and CI (`gdd status`, `gdd new-change`, `gdd launch`, `gdd report`...);
- a **TUI** for the human (`gdd` with no arguments in a terminal): changes by phase, a sitting inbox to approve scenarios, red zones, red plans and releases, a live view of a running change, and token reports.

It replaces the Python bootstrap kit in `gdd-kit/`, which sits on top of Spec Kit. `gdd` does not depend on Spec Kit; it borrows its good ideas (one data sheet per agent, commands as templates rendered per agent, resumable runs with state on disk).

## Who it is for

Developers who work with several coding agents (Claude Code, Codex, OpenCode...) and want agility where risk is low, robustness where it is high, and as few human stops as possible. The first user is the author; it will be published as open source.

## Platform and technology

- Platform: command line and terminal UI, Linux and macOS first.
- Technology: Rust (single static binary, no Python or Node needed). Details in `docs/adr/0001-technology.md`.
- Gherkin scenarios run with cucumber-rs.

## What would be serious if it failed (sensitive areas, in words)

These become red zones at the end of Discover, once there are real folders:

1. **Gate integrity.** A gate opening when it should not: an agent approving a human gate, a red change skipping the plan, a Reviewer of the same model family as the Builder, Path Guard missing a protected path. The whole method rests on this.
2. **The user's repository.** Branches, merges and commits that lose or corrupt work, or write outside the repository.
3. **Secrets and permissions when launching agents.** Leaking environment variables or credentials to an agent, or giving an agent more permissions than its role (a read-only Reviewer that can write).
4. **Shell execution.** Gate commands run through the shell: no agent text may ever be interpolated into a command.
5. **The log.** Token and gate records that are wrong or lost (the report is how the method is measured).

## Constraints

- No Spec Kit dependency in the final tool. The bootstrap kit stays only until `gdd` can run its own gates.
- English in the code, files and commands; the agents talk to the human in the language set in `gdd/project.toml`.
- Few, mature dependencies (broad community support).

## MVP and kill rule

To be decided in Discover.

## Open questions

- Command layout and naming for the CLI.
- How the TUI reads a running workflow (polling the run state or events).
- Packaging: crates.io, GitHub releases, Homebrew.
