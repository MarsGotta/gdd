# Roadmap · gdd 0.1 (MVP)

What the first version will have, and where each item stands. The reasons behind each decision
live in `docs/discover/2026-10-24-mvp.md` and `docs/adr/`. This file is the living list: every Learn
updates it.

Status: **done** · **in progress** · **next** (planned change) · **open** (agreed, no change planned
yet) · **0.2** (after the MVP).

Goal of 0.1: run the Vivarium pilot with `gdd` instead of the Python bootstrap kit.
Kill rule: if `gdd` spends more tokens or asks for more human stops than the kit on the same kind of
change, stop and review before adding more.

## 1. Core

| Item | Status | Where |
|---|---|---|
| Read the configuration (`gdd/*.toml`) | done | core-classify |
| Color, lane and gates of a change, from planned and changed paths | done | core-classify, guard-and-scope |
| Token log and report by change, model, role and phase (`gdd report`) | done | cli-classify-report |
| `gdd classify` | done | cli-classify-report |

## 2. Gates

| Item | Status | Where |
|---|---|---|
| Git reads with fixed arguments, never through a shell | done | guard-and-scope |
| Path Guard by role, with the rules as they were when the build started | done | guard-and-scope |
| Scope check: paths outside the package of an equal or lower color are accepted and listed; a higher color fails G2 | done | guard-and-scope |
| Snapshots, so build mode is judged only for its own work, deleted files included | done | guard-and-scope |
| Run the G2 and G3 commands, save the gate report, log it; only harness commands reach the shell; Path Guard before and after the commands | done | gate-runner |
| Gate reports keep the failure lines of long outputs | done | gate-runner |
| Create changes, record human approvals (G1 with scenario hashes, zones, plan, G4), notes and the Decision Inbox | next | changes-and-approvals |
| G1 refuses hidden examples whose step sentences no visible scenario has | next | changes-and-approvals (done in the kit) |
| Harness self-test | next | changes-and-approvals |

## 3. Agents and the recipe

| Item | Status | Where |
|---|---|---|
| One data sheet per agent: Claude Code, Codex, OpenCode (folder, format, how to launch, JSON output) | next | agents block |
| Launch agents with model and role, capture tokens; the agents use their own logins (subscriptions), not API keys | next | agents block |
| API keys are never passed on to the agents | open | agents block |
| Permissions by role (read-only Reviewer, Builder without the harness) | next | agents block |
| Agents cannot edit GDD state (`.gdd/`): Claude Code | done in the kit | port in the agents block |
| Same for Codex and OpenCode | open | agents block |
| The fixed recipe: Build (test mode, then build mode) → G2 → G3 → Reviewer → merge → pre → G4 pause and resume → promote → observe | next | agents block |
| Replan that can send the change back to test mode; a relaunch with G2 green skips the Builder | done in the kit | port in the agents block |
| The merge commit carries the change's record: color, approvals, G4, Reviewer findings, decisions taken without the human, tokens (the working files stay in `.gdd/`, not versioned) | done in the kit | port in the agents block |
| Agents talk to the human in the project's language (`language` in `gdd/project.toml`) | done in the kit | port in the agents block |
| Mutation testing with a memory cap for test processes (Linux: `prlimit`) | done in the recipes | |
| The same cap on macOS | open | |

## 4. What the human sees

| Item | Status | Where |
|---|---|---|
| Stack catalog: Rust, TypeScript, Python. One default and at most one alternative per gate need, only community-backed tools | done (data) | `docs/discover/2026-10-24-mvp.md` |
| Per-stack sheet: gate commands, scenario runner and skeleton, protected step paths, agent skills loaded on demand | next | human block |
| `gdd init`: the Setup form (name, platform, stack, agents found, sensitive areas), same flow with flags and no TTY, `/gdd-setup` finishes what the form left | next | human block |
| TUI Board: changes by phase, color and gate status (read-only) | next | human block |
| TUI Sitting inbox: scenarios, red zones, red plan and G4 demo; approve or ask for changes, always through a confirmation | next | human block |
| A library's demo is the readable run of its scenarios | done in the kit | |
| All the kit's end-to-end cases pass against `gdd` (t0-t13 today) | next | closes the MVP |
| Vivarium pilot with `gdd`, run by a clean agent; compare tokens and stops with the kit | next | after the MVP |

## 5. Open source

| Item | Status |
|---|---|
| English in code, files and commands; MIT OR Apache-2.0 | done |
| CI on GitHub | done |
| Install: crates.io (name `gdd` free on 2026-10-24), GitHub releases, Homebrew | open |
| English user guide (the current guide is in Spanish) | open |

## Open questions

- Hidden examples are readable by a Builder that goes looking for them.
- Approvals and G1 hashes live in `.gdd/` inside the repository; a backup out of the agents' reach (outside the working tree, or signed) is still to design.
- Codex does not report cost (the kit estimates it from the price table in `gdd/models.toml`) and OpenCode does not report the model (the log keeps the requested one).
- CLI naming, how the TUI reads a running change, packaging (from the Brief).

## 0.2

- Live view of a running change.
- Token charts.
- Semgrep (multi-language static analysis).
- More stacks in the catalog.
