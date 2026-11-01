# ADR 0002 · Blueprint contract: what stays in the core

- Status: proposed (Spike `blueprint-contract`)
- Date: 2026-11-01

## Context

`gdd` becomes T-minus (`tminus`): a core that is the method, the same for any kind of project, and
blueprints that bring what is specific to one kind. The pieces, the phases, the four conditions of
a valid gate and the invariants of the core were decided before this spike and are not revisited.
The question: where does the frontier run through the code as it is today?

Measured at commit `1f8ee74` with `rg -c '^'`; production lines are those before `#[cfg(test)]`:

- `crates/gdd-core/src`: 54 files, 44,945 lines, 19,455 of them production.
- `crates/gdd/src/lib.rs`: 5,939 lines, 2,469 production; the TUI files add 5,893.
- 3,620 lines of `gdd-core` name a software specific (pattern of section 6); 1,477 of them are in
  the 36 files that will become core, whole or in part.

No code was compiled: the Builder of a spike may only run `gdd g2`, `gdd guard`, `gdd note` and git
reads. The one uncertain claim (git moves behind the trait and the approval hashes stay as they
are) is proved by reading the code paths in section 5; step 2 of section 7 proves it compiled.

## Decision

Two crates in this repository, `tminus-core` and `tminus-blueprint-software`, with the interface
below; no plugins yet. The core keeps the engine and every invariant; the blueprint brings every
concrete gate, its locks, and everything about git, Gherkin, cargo and releases.

### 1. Module map

Core moves to `tminus-core` whole, blueprint to `tminus-blueprint-software` whole, mixed is split.
Lines are production / total, biggest first.

| Module | Lines | Class | What goes where |
|---|---|---|---|
| `recipe/mod.rs` | 2,154 / 6,128 | mixed | function by function in 1.1 |
| `gates/` (11 files) | 4,082 / 9,470 | mixed | function by function in 1.2 |
| `agents/` (8 files) | 1,790 / 4,037 | core | `render.rs` (243) mixed: rendering core, stack instructions from the blueprint |
| `init/` (5 files) | 1,215 / 2,485 | mixed | `roles.rs` core; `bootstrap.rs`, `skeletons.rs` blueprint; `mod.rs`, `files.rs`: `tminus/` files core, AGENTS.md, `.gitignore`, `git init` blueprint |
| `changelog.rs` | 1,139 / 2,321 | blueprint | extra |
| `git.rs` | 628 / 1,688 | blueprint | the only place git runs |
| `repo/` (2 files) | 613 / 1,670 | blueprint | `integrate`; `adr_only` is the spike's |
| `change.rs` | 803 / 1,662 | mixed | record, notes, approvals, spent, park: core; `branch`, `branch_at`, variants: blueprint |
| `stacks/` (2 files) | 546 / 1,362 | blueprint | the stack sheets |
| `flow.rs` | 606 / 1,232 | core | time report; its 39 software lines name release steps |
| `learn.rs` | 487 / 1,163 | mixed | repeated polish notes (Harden): core; `[learn]` tools: blueprint |
| `versions.rs`, `delivery.rs`, `upgrade.rs`, `adopt.rs` | 1,663 / 3,789 | blueprint | extras: versions, DORA, upgrade, adoption into known debt |
| `inbox.rs` | 525 / 1,150 | mixed | Decision Inbox, explainer: core; debt, exceptions, variants sections: blueprint |
| `status.rs` | 473 / 937 | mixed | phase, changes, log: core; exceptions and debt lines: blueprint |
| `recipe/queue.rs` | 353 / 910 | mixed | cap, the four locks, overlaps: core; `main_moved`, `take_main`, `restarted`: blueprint `refresh` |
| `rules.rs` | 343 / 786 | mixed | rule per color and lane, spike budget, limits: core; `Acceptance`, command time: blueprint |
| `guard.rs` | 316 / 716 | mixed | Path Guard, scope, hashes: core; its git calls: blueprint `touched`, `rules_at` |
| `classify.rs` | 154 / 416 | mixed | highest color, declared minimum, lane, level: core; `features`, `g1`: blueprint (lock items) |
| `recipe/worktree.rs` | 145 / 325 | blueprint | the `isolate` of the software blueprint |
| `zones.rs` | 152 / 278 | core | the area map: patterns over item ids (section 5) |
| `exec.rs` | 163 / 275 | core | runs the command locks a human approved |
| `state.rs` | 48 / 144 | mixed | the state root: core; finding it with `git::Dirs`: blueprint |
| `session.rs`, `log.rs`, `report.rs`, `config.rs`, `recipe/state.rs`, `lib.rs` | 1,057 / 2,001 | core | |

Production lines: core 3,915 (18 files), mixed 9,464 (18 files), blueprint 6,076 (18 files).

`crates/gdd/src/lib.rs` becomes the `tminus` binary:

| Lines | Part | Class | Note |
|---|---|---|---|
| 1-192, 698-1006, 2103-2172 | `Outcome`, `Voice`, `run`, `run_in`, agent refusals, `record_gate`, `note`, inbox | core | |
| 193-697 | `Cli`, `Command` and arguments | mixed | `merge`, `release`, `changelog*`, `ci`, `bootstrap`, `adopt`, `stacks`, `upgrade`, `board`: blueprint |
| 1007-1408 | reports, classify, guard, snapshot, gate, selftest, CI | mixed | DORA and CI: blueprint |
| 1409-1675 | prepare, merge, release, versions, launch, resume, `in_worktree` | mixed | prepare, launch, resume core; `in_worktree` becomes `isolate` |
| 1726-2102 | `ProjectSteps` (`impl recipe::Steps`), snapshots, hand edits, `new_change` | mixed | today's adapter (section 2); branch and variants: blueprint |
| 2173-2247 | changelog commands | blueprint | |
| 1676-1725, 2248-2469 | agents, stacks, status, adopt, learn, loaders, `main_file` | mixed | "the rules as main has them" is `rules_at` |

#### 1.1 `recipe/mod.rs`, function by function

Tests (2155-6128, 3,974 lines) move with what they test.

| Lines | Items | Class | Note |
|---|---|---|---|
| 1-149 | doc, constants | mixed | `IN_PRE`, `G4_REQUEST`, `CONFLICTS`: blueprint words |
| 151-214, 286-362, 437-483, 837-874 | `Decided`, `Snapshot`, `Said`, `Answer`, `Stop`, `Ended`, `parked_unstarted`, `warned`, `answered`, `answers_g4` | core | |
| 216-284 | `Steps`, `Project` | mixed | section 2; `worktree`, `work`, `main` go to `Place` |
| 364-422, 533-625 | `launch`, `parked_snapshot`, `earlier_work`, `hand_committed`, `not_accepted`, `hand_noted` | mixed | admission, park snapshot, "a person's edits are theirs, the agents' count once gates accept them": core; worktree, commits: blueprint |
| 627-695 | `merge`, `on_latest_main`, `conflicts` | mixed | "target moved: gates before integrating again": core; git merge: `refresh` |
| 697-784, 876-898, 1076-1118 | `resume`, `ok_at_g4`, `g4_request` | mixed | roles, pause, KO, polish, approval, the request: core; spike lock, variants, promote, the request's words: blueprint |
| 424-435, 485-531, 786-835, 846-867, 900-989, 1143-1171, 1195-1215 | `to_main`, `work_committed`, variants, worktrees, `spike_ok`, failed promotions, `promoted`, `observed` | blueprint | |
| 991-1074, 1120-1141, 1173-1193 | `polish_pass`, `polish_noted`, `decided_of`, `saved_in_state`, `finish`, `pause_at_g4` | core | polish is an answer of the approval level |
| 1217-1281 | `Run`, `Run::launched`, `Run::polished` | mixed | order Lock, Cross, integrate: core; spike branches: blueprint |
| 1283-1432, 1699-1800, 2082-2099 | spike rounds, `variant*`, `compared`, `for_test_mode`, `state_left` | blueprint | |
| 1434-1520, 1542-1592 | `g2`, `call`, `spent_on`, `within_budget`, `build`, `still_red`, `rounds` | core | guarded agent call, budget, counted attempts, replan |
| 1522-1540, 1594-1697 | `test_mode`, `integrate`, `merged`, `g3`, `review`, `reviewed` | mixed | Lock step, gates before integrating, guard with one fix: core; merge, pre, findings on step files: blueprint |
| 1801-2080 | reviews kept, `spent`, `g2_failures`, `Kind`, `State`, `allowed` | core | "the agent cannot change its locks": 200 lines |
| 2101-2153 | `Tree`, `working_tree`, `differ` | mixed | the guard only reads: core; the path list from git: blueprint |

Counted function by function (finer than these rows): about 826 lines core, 654 blueprint, 459 mixed.

#### 1.2 `gates/`, function by function

| File | Lines | Class | Split |
|---|---|---|---|
| `mod.rs` | 893 / 2,028 | mixed | Core: `Report`, `Timed`, `Failure` (323-566), `save`, `keep` (573-593), the order in `run` (633-703: Path Guard, commands, Path Guard, scope), `run_commands` (735-763), `kept`, `as_shown` (782-860). Blueprint: `Gate::{G2, G3}` (98-139, becomes `GateId`), `Commands` (141-218), `run_of_kind` (595-612), `is_mutation`, `count_scenarios` (765-779, 862-892), `exceptions` (691-700), `GDD_FEATURES`, `GDD_DIFF_FILES`. |
| `approve.rs` | 776 / 1,721 | mixed | Core: `record`, `may_record` (150-283), `open_g0` (285-316, "Gates phase closed"), `approve_zones`, `red_hash` (318-368, 623-723), `record_on_change`, `g0_open` (378-414, 568-621), the hashing in `seal` (433-463). Blueprint: which files a lock seals (441-448), `unknown_sentences`, `View` (465-566). `Gate` G0-G4 (82-148) gives way to phases and gate ids. |
| `selftest.rs` | 453 / 1,041 | mixed | Core: `run`, `save` (149-197), "every gate seen to close". Blueprint close tests: checks 1 (199-414), 3 (426-433), 4 (435-453). Check 2 (416-424): core for the lock rules, blueprint for CI. |
| `debt.rs` | 442 / 1,041 | mixed | Core: the ratchet (ceilings, `lowered`, `pay`). Blueprint: counting command output, `RISK`, `NEVER_DEBT`, `adopt`. |
| `prepare.rs` | 281 / 803 | mixed | Core: G0, zones, G1, plan, Reviewer family, the decision (77-217). Blueprint: branch check (103-109), `relaunched`, `builder_may` (219-271). |
| `verdict.rs` | 156 / 329 | core | `VERDICT: OK/KO`, `NEXT: test/build` (back to Lock, or Cross again) |
| `release.rs`, `exceptions.rs`, `phrasing.rs`, `ci.rs`, `spike.rs` | 1,081 / 2,507 | blueprint | release and rollback; a `security` check; a Lock check (Gherkin); CI; the spike's gate |

### 2. Adapter interface

`Steps` (`recipe/mod.rs:218-260`, 14 methods) is already the seam: the recipe tests drive it with a
fake (`Played`, `recipe/mod.rs:2211`) and `ProjectSteps` (`crates/gdd/src/lib.rs:1726-1867`) is the
real one. What becomes of each method:

| `Steps` method | Destination |
|---|---|
| `agent`, `say` | survive as they are, in a core `Host` trait: agents belong to the core |
| `decide` | a core function: recording a decision is the engine's job |
| `prepare` | core admission (phases closed, lock and plan approved, guard family) plus `isolate` |
| `gate` | the core gate engine runs the commands; `check` runs the named checks |
| `guard_test`, `guard_earlier`, `hand_edits`, `hashes`, `snapshot`, `rules` | core code over `touched`, `read`, `rules_at` |
| `merge`, `release` | `refresh` then `integrate`; `release`, `roll_back` |
| `work_in` | gone: `isolate` returns the `Place` the core passes back |

```rust
/// What the core asks a blueprint to do. Only the core calls it, never an agent; every method
/// that changes something can run twice, as a relaunch calls it again.
pub trait Blueprint {
    /// What it declares (section 3); the human approves it with the project's rules.
    fn declaration(&self) -> &Declaration;
    /// A place of its own for `task`, made or reused (today: branch `change/<id>` and worktree).
    fn isolate(&mut self, task: &TaskId) -> Result<Place, Error>;
    /// Lets the place go once the task closed or was rejected; what it kept stays.
    fn release_place(&mut self, task: &TaskId) -> Result<(), Error>;
    /// The mark of the work of `task` now (today: the commit checked out).
    fn mark(&mut self, task: &TaskId) -> Result<Mark, Error>;
    /// What `task` touched since `since`, or against the integration target with `None`.
    fn touched(&mut self, task: &TaskId, since: Option<&Mark>) -> Result<Touched, Error>;
    /// The bytes of `item` in the work of `task`, `None` without one. The core hashes them.
    fn read(&mut self, task: &TaskId, item: &ItemId) -> Result<Option<Vec<u8>>, Error>;
    /// The core's rules as they were at `mark`: loosening them later frees no one.
    fn rules_at(&mut self, task: &TaskId, mark: &Mark) -> Result<Option<Vec<u8>>, Error>;
    /// Keeps the work of `task` as `by` (a role or a person), only `items` when given (a commit).
    fn keep(&mut self, task: &TaskId, by: &str, why: &str, items: Option<&[ItemId]>)
        -> Result<(), Error>;
    /// A named check a gate declares (scenario count, exceptions, a spike's ADR): its failures.
    /// Commands are not checks: the core runs them, with Path Guard before and after.
    fn check(&mut self, task: &TaskId, check: &str) -> Result<Vec<String>, Error>;
    /// Brings in the latest integration target if it moved since `since`: not moved, taken at a
    /// new mark (the gates before integrating run again), or the items in conflict (it parks).
    fn refresh(&mut self, task: &TaskId, since: &Mark) -> Result<Moved, Error>;
    /// Integrates `task` with its record (today: merge commit with trailers, push, pre). The
    /// core holds the integration lock and checked every key first.
    fn integrate(&mut self, task: &TaskId, record: &Record) -> Result<String, Error>;
    /// A release step (today: pre, promote, observe); `promote` only once the approval is met.
    fn release(&mut self, task: &TaskId, step: Step) -> Result<Released, Error>;
    /// Puts the release before back.
    fn roll_back(&mut self, task: &TaskId) -> Result<(), Error>;
}

/// What the core needs from where it runs: the two methods of `Steps` that survive as they are.
pub trait Host {
    fn agent(&mut self, call: &Call) -> (Usage, Result<(), invoke::Error>);
    fn say(&mut self, line: &str);
}
```

One real implementation is not "an abstraction with one user" here: the core's own tests need a
fake blueprint (today `Played`), and a second blueprint is planned.

### 3. Blueprint data format

What the software blueprint declares, shipped in `tminus-blueprint-software` as the stack sheets
are shipped today:

```toml
[blueprint]
id = "software"
format = 1

[[gate]]
id = "works"
checks = "the task does what its approved scenarios say and breaks no unit test"
lock = "task"                            # made to measure: the approved scenarios
lock_items = ["features/**/*.feature"]
lock_items_skip = ["features/regression/**"]
lock_check = "phrasing"                  # hidden examples reuse visible sentences
named_checks = { before_integrating = ["scenario-count"] }
close_test = "planted-failing-test"      # selftest checks 1 and 4
level = "lock"                           # default; the rules set it by risk

[[gate]]
id = "quality"                           # lint, architecture, mutation
lock = "project"                         # stock: the project's commands
close_test = "planted-lint-error"        # new: nothing proves it closes today

[[gate]]
id = "security"                          # vulnerabilities, licences, secrets
lock = "project"
named_checks = { before_integrating = ["exceptions"] }
close_test = "planted-secret"            # new

[[gate]]
id = "release"                           # production does not get worse
lock = "project"
close_test = "rollback-in-pre"           # selftest check 3
level = "lock+guard+approval"

[[question]]                             # added to the core's own in the Blueprint phase
id = "stack"
ask = "Which stacks does the project use?"

[[suggested_area]]                       # the human decides
color = "red"
items = ["{stack.supply_chain}"]
reason = "Dependencies and toolchain: supply chain"

agents = { lock = "commands/build-test.md", cross = "commands/build.md", guard = "commands/review.md" }

[[stack]]                                # concrete locks per technology; python, typescript alike
name = "rust"
sheet = "stacks/rust.toml"               # its [g2], [g3] become stages of the gates above

extras = ["changelog", "versions", "dora", "spike_variants", "board"]   # optional; none deleted
```

This repository's `gdd/commands.toml` as `tminus/gates.toml` (long commands unchanged):

```toml
blueprint = "software"
install = "cargo fetch --locked"

[works.every_round]
unit = "cargo test --workspace --exclude gdd-acceptance"
acceptance = "cargo test -p gdd-acceptance"
[works.before_integrating]
acceptance_full = "GDD_FEATURES= GDD_HIDDEN=1 cargo test -p gdd-acceptance --locked"

[quality.every_round]
lint = "cargo fmt --all --check && cargo clippy --all-targets -- -D warnings -W clippy::cognitive_complexity"
architecture = "... (unchanged)"
architecture-frontier = "... (section 6)"
[quality.before_integrating]
mutation = "... (unchanged)"            # only where the level asks for it

[security.before_integrating]
lockfile = "cargo metadata --locked --format-version 1 > /dev/null"
security = "cargo deny --locked check advisories bans sources"
dependencies = "cargo deny --locked check licenses"
secrets = "... (unchanged)"

[release]                               # unchanged
[harden]                                # was [learn]: duplication, unused
[close_test.works]                      # was [selftest]: file, content
[count]
architecture-frontier = ["{n} software lines in the core"]
```

The empty `types` and `quality` go; `configured` drops them already (`gates/mod.rs:210-218`).

`gdd/rules.toml` as `tminus/rules.toml`, the core's part, then the blueprint's:

```toml
limits = { attempts = 3, replans = 1, guard_fixes = 1 }   # builder_retries, replans, reviewer_fixes

[level.green]              # level of a task's gates by color and lane
fast = "lock"
full = { level = "lock+guard", new_lock = "lock+guard+approval" }   # g4_if_g1
[level.yellow]
fast = "lock+guard"
full = { level = "lock+guard", new_lock = "lock+guard+approval" }
[level.red]
full = "lock+guard+approval"   # the core's floor: it refuses less, and asks for the plan too

[protected]                # the core adds tminus/** and .tminus/** whatever this says
all = ["bin/**", ".github/**", ".claude/**", ".agents/**", ".opencode/**", "AGENTS.md", "CLAUDE.md", "GDD.md"]
lock = ["crates/*/src/**"]                                  # was builder_test
cross = ["features/**", "crates/gdd-acceptance/tests/**"]   # was builder_build
guard = ["**"]                                              # was reviewer

[software]
main_branch = "main"
push_main = false
versions = "manual"
tests_first = { green = { fast = false, full = true }, yellow = true, red = true }
mutation = { green = false, yellow = true, red = true }
lock = { dir = "features", regression = "features/regression", required = true }   # [acceptance]
```

`gdd/zones.toml` as `tminus/areas.toml`: `[[zone]]` becomes `[[area]]` and `paths` becomes
`items`; the default, colors and reasons stay. The harness area reads `tminus/**` and
`.tminus/bin/**`.

### 4. G0-G4, piece by piece

| Behavior today | Where | Destination |
|---|---|---|
| G0: Project Brief written | `approve.rs:301` | core concept: the Blueprint phase answered |
| G0: self-test 4/4, opened by the machines | `approve.rs:295-300` | engine rule: the Gates phase closes once every gate was seen to close |
| Checks 1, 3, 4: a planted failing test turns G2 red, rollback works in pre, zero scenarios fail | `selftest.rs:199-414, 426-453` | software gates: close tests of `works`, `release`, `scenario-count` |
| Check 2: a protected file is blocked | `selftest.rs:416-424` | engine rule; its CI half: software gate |
| Red zones approved by a human with a hash; changed, approved again | `approve.rs:333-368, 597, 640` | core concept: the human decides what is sensitive |
| A spike starts before the red zones are approved | `prepare.rs:86` | software extra |
| New or changed scenarios need G1 | `classify.rs:106-113` | software gate picks the lock items; engine rule: a new lock needs approval |
| G1 seals the scenarios (SHA-256); a sealed one changed fails | `approve.rs:433-463`, `guard.rs:209-215` | engine rule (invariant) |
| Hidden examples reuse visible sentences | `approve.rs:465-483`, `phrasing.rs` | software gate: a Lock check |
| G1 means full lane and G4 when `g4_if_g1`; a red plan needs a human | `classify.rs:115, 131-132`, `prepare.rs:99` | approval level: `new_lock`, and the core floor in red |
| `[g2]` commands every round, `[g3]` before merging | `gates/mod.rs:633-703` | software gates `works`, `quality`, `security`, by stage |
| Path Guard before and after; scope in build mode | `gates/mod.rs:665-690`, `guard.rs:270` | engine rule: checked without trusting the agent; risk from what was touched |
| Known debt warns; its count only goes down | `debt.rs` | engine rule: gates only get stricter |
| Mutation only where the color asks; `GDD_FEATURES`, `GDD_DIFF_FILES` | `gates/mod.rs:745-752` | software gate `quality` |
| Retries, replans, then park; G3 red gets one fix | `recipe/mod.rs:1542-1592, 1639-1650` | engine rule: counted attempts, stop and ask |
| A relaunch already green skips the Builder | `prepare.rs:228-252` | engine rule: a key that holds is not earned again |
| The report `g2.json`/`g3.json`, logged | `gates/mod.rs:580-593` | core concept: the key and its record |
| The scenario count; hidden examples only in G3; exceptions need a reason | `gates/mod.rs:767-779`, `commands.toml`, `exceptions.rs` | software gates: named checks of `works` and `security` |
| Main moved: G3 again on the latest main | `recipe/mod.rs:665-683` | engine rule; the merge itself is `refresh` |
| The Reviewer, one fix, quick or deep, another model family | `recipe/mod.rs:1655-1697`, `prepare.rs:162`, `rules.rs:107` | core concept: the guard |
| The Reviewer only reads | `recipe/mod.rs:2101-2153` | engine rule (invariant) |
| The explainer, "How it works" | `inbox.rs`, `g4-explainer.feature` | core concept: approval valid only for what was seen |
| Merge, then pre | `recipe/mod.rs:1619-1635`, `repo/merge.rs` | software: `integrate`, `release` |
| The pause at G4 and its request; KO rejects; polish runs one more round with the notes | `recipe/mod.rs:765-783, 996-1051, 1080, 1175-1193` | approval level; the request's words: software |
| Only a human answers G4; answers under the merge lock | `approve.rs:277`, `recipe/mod.rs:723-736` | engine rule (invariant) |
| OK promotes and observes; a failed promotion keeps the pause | `recipe/mod.rs:881-989` | software `release`; the approval: core |
| A spike's OK brings only its ADR | `recipe/mod.rs:907-935` | software extra |

### 5. What was touched

```rust
/// What a task touched: the only input the core judges risk from. Each item once, by id.
pub struct Touched {
    pub items: Vec<Touch>,
}

pub struct Touch {
    /// A stable id, segments joined by `/` (software: the path from the project root).
    pub item: ItemId,
    /// Added, changed or removed: all three count as touched.
    pub how: How,
}
```

The core does the rest: the color is the highest color of the areas whose patterns match a touched
item (`**` crosses segments, as `zones::glob_matches` does today), raised by the declared minimum,
never lowered by it. Lock items are those the declaration's `lock_items` match; protected items,
those `[protected]` matches for the role. The area map stays core data, approved and hashed, so
risk never depends on blueprint code: the blueprint only says what was touched.

The software blueprint fills it from git: `touched(task, Some(start))` is `Git::changed_since` (the
branch against the build start, untracked files included), `touched(task, None)` is
`Git::changed_from(main, branch)`, and an `ItemId` is the path.

The hashes stay as they are. In `guard.rs`, git only lists paths (`changed_since` at 195, 240 and
288; `is_repository` and `changed_from` at 310-314) and reads the rules where the build started
(`file_at` at 58). No hash goes through it: `file_hash` (`guard.rs:123-137`) is the SHA-256 of the
bytes on disk, or `deleted`; Path Guard compares sealed scenarios with it (`guard.rs:209-215`);
`seal` hashes the bytes `View::read` gives (`approve.rs:456-461`), from disk or `git.file_at`
(`approve.rs:510-522`); `red_hash` hashes the zone tables (`approve.rs:640`). With `touched`, `read`
and `rules_at`, every hash keeps its input bytes and its formula: records sealed today stay valid.

### 6. Frontier metric

```sh
grep -HciE '\bgit\b|branch|merge|worktree|\.feature|gherkin|scenario|release|promot|cargo' \
  $(cat gdd/core-files.txt) \
  | awk -F: '{n += $2} END {print n + 0 " software lines in the core"; exit n > 0}'
```

`gdd/core-files.txt` lists the 36 core and mixed files of section 1. Comments and tests count: a core
test that needs a git repository depends on git. Today: **1,477 lines**: `recipe/mod.rs` 581,
`gates/mod.rs` 130, `gates/approve.rs` 118, `recipe/queue.rs` 67, `init/mod.rs` 67, `guard.rs` 66,
`change.rs` 66, `inbox.rs` 54, `state.rs` 51, 277 in the other 27 files.

The ratchet reuses known debt: the G2 command `architecture-frontier` fails while the number is
above 0, `[count]` reads it, and `gdd/debt.toml` records 1,477, so "a change that reports more fails
the gate, and one that reports less lowers it when it merges" (`debt.rs:8-10`); `can_owe` accepts
the name (`debt.rs:285`). Both files are harness: a human writes them, no agent shortens the list.
At the crate split the list goes and `architecture` takes over: `cargo tree -p tminus-core`
must not show `tminus-blueprint-software`.

### 7. Order of extraction

Each step is a red Refactor, behavior unchanged, main usable after it. The approved steps reach
`gdd-core` only through `version_line`, `classify`, `Change`, `ZoneMap::from_toml`, `default_table`
(`acceptance.rs:49-122`), `stacks::sheet` (814) and `recipe::worktree::folder` (1219); the rest goes
through the CLI. Those names stay re-exported, so no approved step changes.

| # | Change | What moves | Scenarios that guard it |
|---|---|---|---|
| 1 | `frontier-ratchet` | the metric, its count and debt | `ratchet`, `selftest-debt`, `gates` |
| 2 | `touched-items` | `Touched`, `ItemId`, `Mark`; guard, classify, gates and seal take them; the git calls of `guard.rs` go to one software module; proves section 5 | `guard`, `role-guard`, `scope`, `classify`, `cli-classify`, `core-levels`, `approvals`, `park-recovery` |
| 3 | `blueprint-workspace` | `Blueprint` with `isolate`, `mark`, `touched`, `read`, `rules_at`, `keep`; `Steps` loses `work_in`, `hand_edits`, `guard_earlier` | `recipe`, `parallel-launch`, `park-recovery`, `state-root`, `run-steps`, `resume-retry`, `one-gdd` |
| 4 | `blueprint-integrate` | `refresh`, `integrate`, `release`, `roll_back`; `Steps` loses `merge`, `release` | `merge-queue`, `recipe`, `resume-retry`, `sitting-g4`, `delivery-report`, `versions` |
| 5 | `gates-as-data` | `GateId`, gate declarations, stages, named checks; old names read as aliases | `gates`, `gate-details`, `gate-hygiene`, `mutation-speed`, `security-exceptions`, `fix-rounds`, `ratchet`, `spike-flow` |
| 6 | `phases-and-levels` | G0 becomes the Gates phase (a close test per gate), G1 the Lock phase, G4 the approval level | `approvals`, `selftest-debt`, `steps-reuse`, `g4-explainer`, `sitting`, `sitting-g4`, `core-levels`, `review-depth`, `entry-zones` |
| 7 | `extras-to-blueprint` | changelog, versions, upgrade, DORA, adopt, spike and variants, board | `changelog*`, `versions`, `upgrade`, `delivery-report`, `init-adopt`, `spike-flow`, `spike-variants`, `board` |
| 8 | `crate-split` | the two crates; the frontier is 0 and leaves the debt | every scenario (G3), `architecture` |
| 9 | `rename-tminus` | `gdd` to `tminus`, `gdd/` to `tminus/`, `.gdd/` to `.tminus/`, with an upgrade | `upgrade`, `one-gdd`, `state-root`, `init`, `kit-retire` |

### 8. Risks and open questions

1. **Two gates were never seen to close.** The self-test proves `works`, the lock rules, release
   and the scenario count; not `quality` or `security`. Recommendation: step 6 gives each a close
   test (a planted lint error, a planted fake secret) before they count as valid.
2. **How much the core trusts the blueprint.** If the blueprint gave levels or protected the
   harness, the core would delegate an invariant to it, protected zone or not. Recommendation: the
   blueprint lists touched items and nothing more (section 5), and the core protects `tminus/**`,
   `.tminus/**` and the sealed lock items itself.
3. **The red-zone hash.** `red_hash` (`approve.rs:640-713`) hashes the tables as written. The rename
   changes the harness zone, so every project approves its red zones once more. Recommendation:
   accept it in step 9 through `tminus upgrade`; keep the formula.
4. **Old names in the state** (`gates.G1`, `g2.json`, `gate: "G2"`, `phase: "2-build"`), which the
   reports read. Recommendation: from step 5, write new names and read old ones as aliases, always.
5. **The blueprint's commands on the CLI** (`merge`, `release`, `changelog`, `ci`).
   Recommendation: the blueprint registers them under the same names; docs and agents keep working.
6. **Spikes and hidden examples** may be core in any kind of project. Recommendation: keep both in
   the software blueprint now and look again with a second blueprint.
7. **The state root is found through git** (`state.rs`, `git::Dirs`). Recommendation: `Place`
   carries it, so the core never asks git where its state lives.
8. **Section 5 is proved by reading, not compiled.** Recommendation: step 2 goes first and proves
   it, guarded by `approvals`, `guard` and `role-guard`.

### Summary

Today the tool mixes two things: the rules of the method, which hold for any project, and the
habits of software projects (git, branches, merges, test scenarios, releases). Measured in the
code, about a fifth is pure method, a third pure software, and half mixes both. The decision keeps
the method in a core that never sees a file or a branch, and moves everything about software into a
"software blueprint" that tells the core what a task touched and runs the gates it declares. The
core keeps every safety rule: agents cannot change their own checks, only people approve, attempts
are limited. A counter of software words left in the core starts at 1,477 and may only go down;
nine small changes take it to zero, and the tool works after each one.

## Alternatives

- **Installable plugins now, as separate programs** (like Terraform providers). Rejected: with one
  blueprint the protocol would be designed blind. The trait is its first draft.
- **Two crates first, untangle after.** Rejected: one move of 19,455 lines leaves main broken until
  it compiles, and the frontier cannot be measured on the way.
- **A blueprint as data only, every gate a command.** Rejected: isolation, integration, release and
  checks like the scenario count are code. Data declares the gates; the trait does the rest.
- **The blueprint gives the levels.** Rejected: the core would delegate an invariant (risk 2).

## Consequences

- The next changes are the nine of section 7, each a red Refactor with its scenarios unchanged.
- `Steps` goes in steps 3 and 4; `Host` and `Blueprint` replace it, and the recipe tests (3,974
  lines) drive a fake blueprint instead of `Played`.
- The first harness change, `gdd/core-files.txt` and the frontier entry in `gdd/debt.toml`, needs
  the human's approval.
