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
| Create changes, record human approvals (G1 with scenario hashes, zones, plan, G4), notes, the Decision Inbox and status | done | changes-and-approvals |
| G1 refuses hidden examples whose step sentences no visible scenario has | done | changes-and-approvals |
| Harness self-test (`gdd selftest`, 4 checks) | done | kit-retire (2026-10-26) |
| Retire the bootstrap kit here: `gdd selftest`, `gdd ci`, `gdd session`, the change from `GDD_CHANGE`; `bin/gdd` runs only `gdd`; CI builds `gdd` from main and judges the change with it; `gdd-kit/` deleted | done | kit-retire (2026-10-26) |
| Interactive sessions of Codex and OpenCode in the log (`gdd session` reads Claude Code's only, as the kit did) | open | |
| Code comments in `agents/usage.rs` and `recipe/mod.rs` still cite the deleted kit files as reference | done | resume-retry (2026-10-27) |

## 3. Agents and the recipe

| Item | Status | Where |
|---|---|---|
| One data sheet per agent: Claude Code, Codex, OpenCode (folder, format, how to launch, JSON output); GDD commands rendered per agent, Spec Kit gone | done | agent-sheets (2026-10-26); `gdd agents install|check` |
| Messages of `gdd prepare` and `gdd status` name the GDD commands, not Spec Kit | done | agent-launch (2026-10-26) |
| Launch agents with model and role, capture tokens; the agents use their own logins (subscriptions), not API keys | done | agent-launch (2026-10-26): gdd starts each agent's own program, no Python wrapper |
| API keys are never passed on to the agents (`*_API_KEY`, `*_TOKEN`, `*_SECRET`, `*_PASSWORD`; `[env] pass` for the ones a project passes on purpose) | done | agent-launch (2026-10-26) |
| Permissions by role (read-only Reviewer, Builder without the harness), as data in the agent sheets | done | agent-launch (2026-10-26) |
| Agents cannot edit GDD state (`.gdd/`): checked by gdd itself around every agent call, state put back and the change parked | done | agent-launch (2026-10-26) |
| Same for Codex and OpenCode, and the Reviewer only reads (working tree compared by type, permissions, content and link target); the agent's processes are killed when its turn ends | done | agent-launch (2026-10-26) |
| Single recipe steps: prepare (entry checks, models per role), park, merge with the change's record, release pre/promote/observe with rollback | done | run-steps (2026-10-26) |
| The fixed recipe: Build (test mode, then build mode) → G2 → G3 → Reviewer → merge → pre → G4 pause and resume → promote → observe; Spec Kit stops running it | done | recipe-run (2026-10-26); `bin/gdd launch|resume` run it |
| `gdd resume --g4 ok` keeps the pause until the promotion passes, so a failed promotion can be retried; a change caught mid-promotion (`promoting`) parks instead of guessing; a change already in production is only observed | done | resume-retry (2026-10-27) |
| Kit e2e t8 fails when the suite runs from this repository (passes in the pilot repository) | no longer applies | kit-retire removed the kit here; the suite runs in the pilot |
| A relaunch with G2 green skips the Builder, checked under Path Guard | done | run-steps; kit 0.5.5 |
| Replan that can send the change back to test mode | done | recipe-run |
| The merge commit carries the change's record: color, approvals, Reviewer findings, decisions taken without the human, tokens (the working files stay in `.gdd/`, not versioned) | done | run-steps |
| Agents talk to the human in the project's language (`language` in `gdd/project.toml`) | done | agent-sheets (command templates) |
| Mutation testing with a memory cap for test processes (Linux: `prlimit`) | done in the recipes | |
| The same cap on macOS | open | |

## 4. What the human sees

| Item | Status | Where |
|---|---|---|
| Stack catalog: Rust, TypeScript, Python. One default and at most one alternative per gate need, only community-backed tools | done (data) | `docs/discover/2026-10-24-mvp.md` |
| Per-stack sheets (Rust, TypeScript, Python): gate commands, scenario runner, self-test file, protected step paths, tool catalog, per-stack instructions for the agents (`gdd-stack-<name>`); one or more per project; `gdd stacks`, `show`, `compose` | done | stack-sheets (2026-10-27) |
| Project skeleton per stack, written by `gdd init` when the folder has no project of that stack; `gdd bootstrap` (first install of each stack, commits the lock files); per-stack CI setup; Python on uv; every gate command run for real on new Rust, TypeScript, Python and combined projects (self-test 4/4) | done | stack-skeletons (2026-10-27) |
| Security exceptions: `gdd status` and G4 list the active ones (osv-scanner.toml `[[IgnoredVulns]]`, deny.toml `[advisories] ignore`); one without a reason is refused; they live in red-zone files, so adding one is a red change | done | security-exceptions (2026-10-27) |
| `gdd init` from flags, without a screen: project basics, composed stack sheets, protected paths, first zone map (harness and supply chain red), roles per agent (Reviewer of another family, or one family accepted with its date), Brief skeleton, `bin/gdd`, CI, AGENTS.md/CLAUDE.md when absent, `gdd/generated.toml`; never overwrites, never writes through links or files with other names, commits only its own files | done | init (2026-10-27) |
| `gdd init` form screen; `gdd status` notices a second model family to change the Reviewer | done | init-form (2026-10-27): red, not yellow (ratatui 0.30 + crossterm 0.29 touch the lock file) |
| `gdd upgrade`: a new gdd version proposes its changes to the files it wrote once (three-way merge against what it generated then; untouched files updated, edited ones merged, conflicts marked), as a change approved at a sitting; `init` records which version generated each file | next | gdd-upgrade (after the pilot) |
| One state for every worktree: all `.gdd/` paths resolve to the main checkout (git common dir), so status, records, log and gates agree from any worktree | done | state-root (2026-10-27) |
| Each change runs in its own git worktree (sibling folder `<repo>.worktrees/<id>`, configurable); the main checkout stays on main; the agents-state check is per change; log appends under a lock | done | parallel-launch (2026-10-27) |
| Several changes at once: changes whose planned paths overlap cannot run together, `[parallel] max` (default 2) caps changes building at once; merges one at a time, each change takes the latest main and passes G3 again before merging | done | merge-queue (2026-10-27): first Change built in its own worktree, alongside init-form |
| A person's writes to shared state (red zones, G0, self-test) wait for the agent calls under way instead of parking them; approving the red zones as they are writes nothing | done | shared-writes (2026-10-28) |
| Adoption of a repository with code: `gdd init` runs every gate once, reports what fails, proposes Refactor changes ordered by risk; failing commands are known debt (warn, do not block) until a Refactor turns each green | done | init-adopt (2026-10-27): `gdd adopt`, `gdd/debt.toml` |
| TUI Board: changes by phase, color and gate status (read-only) | next | human block |
| TUI Sitting inbox: scenarios, red zones, red plan and G4 demo; approve or ask for changes, always through a confirmation | next | human block |
| Changelog from the changes: one user-facing line per change, proposed at the sitting, carried as a trailer of the merge commit (`Changelog:`, `Breaking:`); `gdd changelog` writes `CHANGELOG.md` in Keep a Changelog format with an Unreleased section; Spike, No-code and invisible refactors are left out | next | changelog |
| Versions: on promotion, the next SemVer from the change (Hotfix → patch, Feature → minor, breaking → major), a git tag, Unreleased moves to the version; `manual` mode for projects that release in batches (desktop apps) | next | changelog |
| A library's demo is the readable run of its scenarios | done in the kit | |
| Principle «done is not good»: the human is the editor at G4 | done (method) | `docs/discover/2026-10-26-done-is-not-good.md` |
| G4 answers OK, KO or polish (one more Builder round with the human's notes) | done | recipe-run |
| `docs/standards.md`: the project's quality bar, read by the Builder and the Reviewer, fed by Learn | done in the kit | `gdd init` writes a first one |
| Secrets scanning (gitleaks) and the linter's complexity rule in the gates | done in this repository | stack sheets for every stack |
| Duplication warning (jscpd) and unused code in Learn (knip, vulture, cargo-machete) | in the sheets (`[learn]`, stack-sheets); Learn running them: next | learn-tools |
| `gdd report --dora`: DORA's five delivery metrics and GDD's signals (polish rate, parks, Reviewer findings) from the log | next | delivery-report |
| Spikes with variants: N Builders build the same spike, the human picks one side by side | next | spike-variants |
| All the kit's end-to-end cases pass against `gdd` (t0-t15, from the pilot repository, where the suite lives) | next | closes the MVP |
| Vivarium pilot with `gdd`, run by a clean agent; compare tokens and stops with the kit | next | after the MVP |

## 5. Open source

| Item | Status |
|---|---|
| English in code, files and commands; MIT OR Apache-2.0 | done |
| CI on GitHub | done |
| Install: crates.io (name `gdd` free on 2026-10-24), GitHub releases, Homebrew. The CI written by `gdd init` installs gdd from its git tag `v<version>`: it works once the first version is tagged | open |
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
- Bare repositories and `--separate-git-dir`: today gdd refuses to run in a worktree of a bare repository (or of one whose main checkout git cannot name), because there is no main checkout to keep the shared `.gdd/` state in (state-root, 2026-10-27). Support them with a configurable state folder, so the "one bare repository + one worktree per branch" style works. No urgency.
