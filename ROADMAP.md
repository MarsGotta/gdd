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
| Minimal by default, incremental by choice (Mars, 2026-10-29). Three levels: the core that guards against the agents is always on and no Builder can turn it off (gates G1-G4, the semaphore, Path Guard, hidden examples, a Reviewer of another model family, and mutation testing on red changes); the stack's checks are on by default and can be switched off; extras are opt-in, each by adding its section to a `gdd/*.toml` as `[learn]` works, and none adds a question to `gdd init`. Mutation testing moves to the core for red changes only and becomes opt-in for the rest | done | core-levels (2026-10-29): new projects run mutation testing on red changes only; a rules file that switches off mutation, the Reviewer or the human G4 for red changes is refused, a missing key counts as on; `gdd status` warns without a mutation command |
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
| `gdd upgrade`: a new gdd version proposes its changes to the files it wrote once on the branch `gdd-upgrade` (three-way merge with `git merge-file` against what it generated then, kept pure in git; untouched files updated, edited ones merged, conflicts marked); the human reviews and merges it; `init` records its answers in `gdd/generated.toml` | done | gdd-upgrade (2026-10-29): a branch the human merges, not a change at a sitting (Mars's call) |
| One state for every worktree: all `.gdd/` paths resolve to the main checkout (git common dir), so status, records, log and gates agree from any worktree | done | state-root (2026-10-27) |
| Each change runs in its own git worktree (sibling folder `<repo>.worktrees/<id>`, configurable); the main checkout stays on main; the agents-state check is per change; log appends under a lock | done | parallel-launch (2026-10-27) |
| Several changes at once: `[parallel] max` (default 2) caps changes building at once (overlapping planned paths only warn since overlap-warns); merges one at a time, each change takes the latest main and passes G3 again before merging | done | merge-queue (2026-10-27): first Change built in its own worktree, alongside init-form |
| A person's writes to shared state (red zones, G0, self-test) wait for the agent calls under way instead of parking them; approving the red zones as they are writes nothing | done | shared-writes (2026-10-28) |
| Overlapping planned paths warn instead of refusing a launch (as GitHub's merge queue admits every pull request); git settles the overlap at the merge, and a conflict parks the change with the way out | done | overlap-warns (2026-10-28): all 21 earlier changes touched the steps file |
| Adoption of a repository with code: `gdd init` runs every gate once, reports what fails, proposes Refactor changes ordered by risk; failing commands are known debt (warn, do not block) until a Refactor turns each green | done | init-adopt (2026-10-27): `gdd adopt`, `gdd/debt.toml` |
| TUI Board: changes by phase, color and gate status (read-only) | done | board (2026-10-28): first change merged by the queue on top of another (changelog) |
| TUI Sitting inbox: scenarios, red zones, red plan and G4 demo; approve or ask for changes, always through a confirmation; first the terminal code the Setup form and the Board each carry moves to one module | done | tui-shared, sitting, sitting-g4 (2026-10-28) |
| Changelog from the changes: one user-facing line per change, proposed at the sitting, carried as a trailer of the merge commit (`Changelog:`, `Breaking:`); `gdd changelog` writes `CHANGELOG.md` in Keep a Changelog format with an Unreleased section; Spike, No-code and invisible refactors are left out | done | changelog (2026-10-28) |
| CHANGELOG.md is the project's: gdd never rewrites it; `gdd changelog` shows, version by version, what the merge commits carry and the file lacks, `--write` adds only that (each line with a comment naming its commit, so a reworded line still counts), `--check` for CI; replaces the marker line, which either lost hand edits or refused an existing changelog | done | changelog-merge (2026-10-28) |
| Versions: git tags `v<semver>`, by default one per promoted change with a changelog line (Fixed/Security patch, other sections minor, breaking major, minor while 0.x; first 0.1.0), tagged only after the promotion succeeds, with a `Release` commit turning Unreleased into the version; `[versions] mode = "manual"` and `gdd release` for projects that release in batches (this repository: v0.1.0 closes the MVP) | done | versions (2026-10-28) |
| A library's demo is the readable run of its scenarios | done in the kit | |
| Principle «done is not good»: the human is the editor at G4 | done (method) | `docs/discover/2026-10-26-done-is-not-good.md` |
| G4 answers OK, KO or polish (one more Builder round with the human's notes) | done | recipe-run |
| `docs/standards.md`: the project's quality bar, read by the Builder and the Reviewer, fed by Learn | done in the kit | `gdd init` writes a first one |
| Secrets scanning (gitleaks) and the linter's complexity rule in the gates | done in this repository | stack sheets for every stack |
| Duplication warning (jscpd) and unused code in Learn (knip, vulture, cargo-machete) | in the sheets (`[learn]`, stack-sheets); `gdd learn` runs them, warnings only, and lists the G4 polish notes | done | learn-tools (2026-10-29) |
| `gdd report --dora`: DORA's five delivery metrics and GDD's signals (polish rate, parks, Reviewer findings) from the log | done | delivery-report (2026-10-28) |
| Changelogs in the common heading styles, not only Keep a Changelog's brackets: `## Unreleased`, `## 0.1.0 - <day>`, `## v0.1.0`, `## [0.1.0](<compare link>) (<day>)`; gdd adds to the file's own Unreleased and writes new versions in the file's style (found on Jezero's CHANGELOG.md: gdd added a second Unreleased) | done | changelog-styles (2026-10-28) |
| Where the time goes: each change's flow time (from its creation to closed) split into active time (agent calls by role and model, gates by command) and waiting (for the human at the sitting and at G4, parked until relaunched, in the merge queue), flow efficiency (active / flow time, Flow Framework), totals per project and per period, to see whether GDD saves time and which step to polish; the log gains what it lacks (the pause at G4, each gate command's duration) | done | flow-time (2026-10-28) |
| The same report as a TUI screen (from the Board: a change's time in its details, the project's in a report view, key `t`) | done | report-screen (2026-10-28) |
| A Reviewer that converges: every finding in one pass, each named with its class of bug (so the Builder fixes every instance, not one line), later rounds review only the new diff and never repeat a finding already settled (from Anthropic's Claude Code review, 2026-10-15: mean reviews to pass 2.08 to 1.32; here the Reviewer says KO in 48% of its reviews and `sitting` needed six rounds) | done | reviewer-converge (2026-10-29): the record keeps every review with the files changed since the one before; `gdd report --dora` says reviews to pass (here, before it: median 2 per change, first review OK in 9 of 37 changes) |
| Learn from the Reviewer too: `gdd learn` lists the classes of findings that repeat across changes (as it lists the G4 polish notes), so they become a line in `docs/standards.md`, a check or a lint rule (Anthropic's `REVIEW.md`: repeated review comments move into the instructions) | next | learn-reviewer (before the pilot: Mars, 2026-10-29) |
| Against comprehension debt: the G4 page shows a short explainer of the part of the code the change touched (how it fits, what pattern it sets), written from the diff, so the human understands what she approves without reading every line (Addy Osmani, "Software Factories, Light and Dark") | next | g4-explainer (before the pilot: Mars, 2026-10-29) |
| Review depth by risk, on by default: a quick Reviewer pass for yellow changes and a deep one for red, set per color in `gdd/models.toml` (Anthropic's Quick, Standard and Deep reviews); cheaper review where a miss costs little | next | review-depth (before the pilot: Mars, 2026-10-29) |
| A ratchet for known debt: `gdd adopt` counts what each failing command reports today (lint warnings, type errors, failing tests) and keeps that count as a ceiling; a change that raises it fails the gate with the difference, one that lowers it tightens the ceiling for good. Today a command with known debt never fails a gate, so a change can add more of the same failure unseen | next | ratchet (before the pilot, a brownfield: Mars, 2026-10-29) |
| Spikes with variants: N Builders build the same spike, the human picks one side by side | next | spike-variants |
| All the kit's end-to-end cases pass against `gdd` (t0-t15, from the pilot repository, where the suite lives) | next | closes the MVP |
| Pilot: adopt `gdd` in Jezero (MarsGotta/jezero: Python, ~13k lines of code and ~9k of tests, its own CI and CHANGELOG.md), a brownfield of moderate size, run by a clean agent; measure tokens, stops and flow time (the human's call, 2026-10-28; replaces the Vivarium pilot) | next | after the MVP |

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
- A Builder round that settles the merge conflicts of a parked change (agent tools do it; today the Orchestrator merges main into the branch by hand).
- Bare repositories and `--separate-git-dir`: today gdd refuses to run in a worktree of a bare repository (or of one whose main checkout git cannot name), because there is no main checkout to keep the shared `.gdd/` state in (state-root, 2026-10-27). Support them with a configurable state folder, so the "one bare repository + one worktree per branch" style works. No urgency.
- `gdd changelog-line` taking a line whose words start with `--` (today `gdd report --time: ...` needs a `--` before the words).
- A flaky unit test from `versions`: `a_promotion_gets_its_version_and_releases_it_once_it_passed` sometimes also hears `waiting for the merge of another change` (another test's child process, forked while the merge lock was held and not yet exec'd, still holds it for a moment). The code is right (it waits); the test should assert the release line, not that no wait was said. Found on sitting's G2, 2026-10-28.
- The sitting at G4 shows the `gdd resume` command with the note quoted as typed: escape quotes and shell characters so a copied command keeps the words (the run itself already passes them intact). Reviewer's suggestion on sitting-g4.
- The `gdd-learn` agent command calls `bin/gdd learn` (agent assets are a red zone: rides on the next red Change that touches them).
- `gdd upgrade` delivered as a change approved at a sitting (needs a recipe lane without a Builder: Path Guard refuses `gdd/**` on change branches).
- `gdd upgrade` for projects set up before `gdd/generated.toml` recorded the answers (MarsGotta/gdd itself): adopt them by writing the record from today's files.
- A separate security review by an agent for changes in red security zones, whose findings block like the Reviewer's (today only the deterministic scanners look at security).
- Evaluate the Reviewer: keep its past findings with what happened to each (fixed, answered as a known limit, a fault of the package, missed and found later) as an eval set, rerun it whenever the Reviewer's model or instructions change, and report its noise (findings that led nowhere).
- Real staged releases: internal use first, then everyone, with the observe step reading real signals (arrives with the pilot's real pre and production; here both are simulated).
- Feature flags for larger projects (a Discover topic first), only for Features that change what users see (Hotfix, Refactor, Migration and No-code go without one, as at Anthropic, where new behavior usually ships behind a flag and an additive change behind one is low risk): deploy separate from release, so a change merges and deploys with its flag off and the G4 becomes turning it on; flags declared in the repository with their kind (release, experiment, ops, permission: Pete Hodgson's categories), the change that owns them and an expiry; G3 runs the scenarios with the flags the change will turn on and again with them off; a release flag at 100% proposes its removal Change, `gdd status` warns past the expiry and a cap limits the live flags. GDD imposes the method, not the tool: OpenFeature (CNCF) as the recommended standard API, with a file in the repository as its first provider.
- Shape checks on by default (level 2, through the stack sheets): lines per module, function length and complexity, layers, with ceilings that only tighten (the ratchet) so an existing project is not asked to fix everything at once. Here `crates/gdd/src/lib.rs` already has about 2,000 lines of code before its tests; another team's agent loop reached 4,000-line modules with every check green and had to be rebuilt.
- Say why an agent call failed: quota, configuration, an empty answer (exit code 0 with nothing done) or a transient error, each with what to do; only the transient one is retried. Today gdd knows only that the agent failed or timed out.
- A check whose object does not exist yet (a lock file, a package) says "pending" in green instead of failing; once it existed, losing it is red. For new projects, next to the stack checks (level 2).
- Several verifiers vote on each Reviewer finding before it reaches the Builder (Anthropic posts a finding only when enough agents agree it is real), opt-in for red changes: it costs one call per verifier.
