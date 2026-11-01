# ADR 0003 · Gate engine: valid gates, levels by risk, gates that only get stricter

- Status: proposed (Spike `gate-engine`)
- Date: 2026-11-01

## Context

ADR 0002 set the frontier: the core defines no gate, a blueprint declares them, the core runs
them. It did not say how the engine works: how it knows a gate is valid, how it sets each gate's
level by risk, how today's colors, lanes and `CORE` become a case of it, and how it enforces the
one invariant not implemented today: gates only get stricter, and loosening one needs the human's
approval. This ADR builds on ADR 0002 and deviates from it only where it says so.

Measured at commit `8394fc0` by reading the code and with `git log`; no code was compiled (a
spike's Builder may only run `gdd g2`, `gdd guard`, `gdd note` and git reads). Today's engine is
3,913 production lines in nine files of `gdd-core` (`gates/mod.rs`, `approve.rs`, `selftest.rs`,
`debt.rs`, `prepare.rs`, `rules.rs`, `guard.rs`, `classify.rs`, `zones.rs`). What matters here:

- **A change is judged by its own branch's rules.** The gates read `gdd/rules.toml` and
  `gdd/commands.toml` from the worktree (`crates/gdd/src/lib.rs:1378-1391`); only the zone map
  comes from main (`lib.rs:1373-1375`, `2353-2359`).
- **Loosening needs nothing.** No code compares two versions of the rules but the red-zone hash
  (`approve.rs:670-680`). Agents gdd starts cannot touch `gdd/**` (`rules.toml:52`), but an edit
  on main takes effect at once, and CI checks only change branches (`gates/ci.rs:99-104`).
- **The floor is four keys.** `CORE` (`rules.rs:282`) refuses `mutation`, `reviewer` or
  `g4_if_g1` false, or `review = "quick"`, in red, when the file is read (`rules.rs:212-215`,
  `296-304`); red always gets G4 and the plan (`classify.rs:131-132`). `tests_first` is not in it.
- **"Seen to close" is one check for every gate, done once.** Self-test check 1 passes when any G2
  command turns red with the planted test (`selftest.rs:251`): it proves `works`, not `quality`.
  Check 3 passes with `simulated = true` and no command (`selftest.rs:426-431`), as this repository
  has it (`commands.toml:28, 36`): it proves no gate closes. G0 reads the result once
  (`approve.rs:309`); nothing runs it again when the commands change (`prepare.rs:89-91`).
- **Data the code ignores.** `g4_if_g1` in a fast lane (`rules.toml:24, 36`; `classify.rs:115`)
  and `[limits] reviewer_fixes` (`rules.toml:16`; `agents/mod.rs:21-30`, the one fix is fixed in
  `recipe/mod.rs:1637-1652`) change nothing.

The harness files changed 22 times since the Setup (`git log -- gdd/rules.toml gdd/commands.toml
gdd/zones.toml`, 2026-10-25 to 2026-11-01), all directly on main. Classified by section 4:

| Kind | Commits | Count |
|---|---|---|
| Loosening in effect | `7c7000f` spike budget from the default 5 to 10 USD; `aa7f639` a mutant that hangs twice counts as caught; `f02db15` four G2 commands lose `--locked`; `6dc70ef` the TUI's theme and texts from yellow (the default) to green | 4 |
| Loosening the engine cannot rule out | `a6787cb`, `c6fdf66`: protected pattern of a folder deleted in the same commit; `ab0d5d6`: `tests/acceptance/**` dropped; `a3b895f`, `ff0d0cc`, `e82d32d`, `9fa8e78`, `5fd9084`, `0e85c1e`, `c92e1eb`: an existing command's text changed | 10 |
| Tightening only | `7c5a6b6`, `3acc5f0`, `37eb30e`, `7d63226`: red areas widened or added; `0d6a31d`: a protected pattern added | 5 |
| Neither | `2aec68a` a comment; `7073a7c` `[learn]`; `c820637` `[versions]` | 3 |

None of the four real loosenings was recorded as one.

## Decision

The engine lives in `tminus-core/src/engine/`: `gate.rs` (declarations, validity, close tests;
from `gates/mod.rs`, `selftest.rs`), `level.rs` (risk, level rules, floor; from `rules.rs`,
`classify.rs`), `key.rs` (keys; from `Report`), `loosen.rs` (new), next to `areas.rs` (`zones.rs`).

### 1. The engine's model

```rust
/// A gate a blueprint declares: what is checked. The core defines none.
/// Its stages run in order (`every_round`, `before_integrating`, release steps), each with the
/// blueprint's named checks and the project's commands for it.
pub struct GateDecl {
    pub id: GateId,         // `works`, `quality`, `security`, `release`; old names alias them
    pub checks: String,     // one sentence the human reads in the Blueprint phase
    pub lock: LockKind,
    pub stages: Vec<Stage>,
    pub optional: BTreeMap<String, Vec<Pattern>>, // run only where a rule says (`mutation*`)
    pub close_test: Option<CloseTest>,            // without one, the gate is unproven
}

pub enum LockKind {
    /// Made to measure: the items `items` match, minus `skip`, sealed in the task's Lock phase
    /// after the named check `check` (software: the approved scenarios, phrasing).
    Task { items: Vec<Pattern>, skip: Vec<Pattern>, check: Option<String> },
    /// Stock: the project's commands for this gate.
    Project,
}

/// How a gate is seen to close: `plant` (an item and its bytes, or a plant the blueprint names,
/// such as a fake secret made at run time) must turn `stage` red, green without it.
pub struct CloseTest { pub plant: Plant, pub stage: StageId }

/// Whether a gate counts. The engine computes it; nothing declares it.
pub enum Validity {
    /// The four conditions hold; `closed` is its last close test, for its lock as it is now.
    Valid { closed: Closed },
    /// It runs and its red blocks, but its green is no key: never closed, or its lock changed.
    Unproven { why: String },
    /// Refused when the rules are read: nothing runs.
    Invalid { why: String },
}

/// How much a gate asks for. Ordered: a higher level is stricter. `Open`: the gate does not run
/// for this risk (no line of today's table uses it).
pub enum Level { Open, Lock, LockGuard, LockGuardApproval }

/// The risk of a task, computed by the core from `Touched` and the approved area map.
pub struct Risk {
    pub color: Color,          // highest area its items match, raised by the declared minimum
    pub new_lock: bool,        // it brings lock items that need sealing (was: G1)
    pub lane: Lane,            // fast only below red, without a new lock, and resumable
    pub reasons: Vec<String>,  // one line per item and per raise (`classify.rs:84-104`)
}

/// One line of the rules: the level of every gate (`gate: None`), or of one, for a color and a
/// lane. A line for one gate overrides the line for every gate.
pub struct LevelRule {
    pub color: Color, pub lane: Lane, pub gate: Option<GateId>, pub level: Level,
    pub new_lock: Option<Level>, // with a new lock (was: `g4_if_g1`); never below `level`
    pub run: Vec<String>,        // optional commands it switches on (was: `mutation`)
    pub guard: Depth,            // how deep the guard looks (was: `review`)
    pub lock_step: LockStep,     // Never | NewLock | Always (was: `tests_first`)
}

/// What the engine decided for a task: sealed in its Lock phase, raised when risk rises.
pub struct Levels {
    pub risk: Risk,
    pub gates: BTreeMap<GateId, GateLevel>, // level, optional commands on, guard depth, why
    pub plan_approval: bool,                // the human approves the plan (the floor, in red)
    pub rules: Hash,                        // the effective rules it is judged by (section 4)
}

/// The key: what one run of one stage of one gate proved, for one mark of the work. It holds
/// while mark, rules and lock are the same: a relaunch already green skips (`prepare.rs:121`).
pub struct Key {
    pub gate: GateId, pub stage: StageId, pub task: TaskId, pub mark: Mark,
    pub rules: Hash, pub lock: Hash, pub level: Level,
    pub proven: bool, // the gate was valid when it ran; an unproven gate's green says `false`
    pub ok: bool, pub passed: Vec<String>, pub failed: Vec<Failure>, pub skipped: Vec<String>,
    pub owed: Vec<Owed>, // known debt held to its ceiling (`debt.rs:259-276`)
    pub judge: Judge,    // Machines | Guard { model, family } | Human
    pub ts: String,
}

/// One loosening between two versions of the rules: the part, before and after.
pub struct Loosened { pub part: String, pub before: String, pub after: String }

/// An approval of the rules; it holds only for `after`. `by` is "human" for a loosening,
/// "machines" for a tightening, "migration" for the baseline (section 6).
pub struct RulesApproval {
    pub before: Option<Hash>, pub after: Hash, pub loosened: Vec<Loosened>,
    pub by: String, pub ts: String, pub note: String,
}
```

What the engine records, all under the state root as today:

| Record | Holds | Was |
|---|---|---|
| `.tminus/gates.json` | the Gates phase, each gate's last close test with its lock hash | `g0.json`, `selftest.json` |
| `.tminus/areas.json` | the human's approval of the sensitive areas, same hash | `zones.json` |
| `.tminus/rules.json` | the approved effective rules, their hash, every `RulesApproval` | new |
| `changes/<id>/change.json` | `levels`, `approvals.{plan, lock, result}`, keys by gate and stage | `decision`, `gates.*`, `g2.json`, `g3.json` |
| `.tminus/log.jsonl` | a line per check and per gate run, with gate, stage, level, judge | `.gdd/log.jsonl` (`gates/mod.rs:533-565`) |

### 2. Validity check

| Condition | On a blueprint's declaration | On a project's rules | When |
|---|---|---|---|
| Decided before the agent starts | shipped data, hashed; no gate is made at run time | the Lock phase seals the task's levels and the rules hash; each run compares (section 4) | load, Lock phase, each run |
| Checked without trusting the agent | every stage has a command or a named check; a key comes from exit codes and check results, never from an agent's words | every item a lock is read from (the gates file, the rules, the lock items) is protected for every agent role: the core adds its own items whatever `[protected]` says | load: refused; each run: Path Guard before and after (`gates/mod.rs:665-680`) |
| The agent cannot touch it | | sealed lock items (`approve.rs:448-480`, `guard.rs:209-215`); the state checked around each call (`recipe/mod.rs:40-44`) | each run |
| Seen to close | a close test declared | the close test passed with the gate's lock as it is now | Gates phase; again when its lock changes |

A gate whose lock is missing, whose stages run nothing, or whose lock lives in an item an agent may
write is **invalid**: the rules are refused when read, as `core_switched_off` refuses them today.

**A close test in data.** `[selftest]` (`commands.toml:42-44`) becomes the close test of `works`,
and the software blueprint gives `quality` and `security` their own:

```toml
[close_test.works]
plant = { item = "crates/gdd-core/tests/gdd_selftest_failing.rs", content = "#[test]\nfn gdd_selftest_must_fail() {\n    panic!(\"gdd selftest\");\n}\n" }
stage = "every_round"
[close_test.quality]              # a badly formatted test file: lint must turn red
plant = { item = "crates/gdd-core/tests/gdd_selftest_lint.rs", content = "fn  badly_formatted ( ) { }\n" }
stage = "every_round"
[close_test.security]             # the blueprint writes a key the secrets scanner matches
plant = { named = "fake-secret" }
stage = "before_integrating"
```

**How the engine runs it.** (1) `isolate` a place for the close test alone, from main's mark; (2)
run the stage: green, or it fails as check 1 does (`selftest.rs:229-234`); (3) `plant`; (4) run
the stage: red, from this gate's own commands; (5) `release_place`; (6) record gate, lock hash and
result. A throwaway place drops today's setting aside and giving back (`selftest.rs:254-350`, 97
lines) and never touches the main checkout. **Deviation from ADR 0002 section 2:** the `Blueprint`
trait gains one method, as no other one writes into a place:

```rust
/// Plants the failure of a close test in the place `isolate` made for `task` alone; the core
/// never plants where a task works. `release_place` takes it away.
fn plant(&mut self, task: &TaskId, plant: &Plant) -> Result<(), Error>;
```

**Unproven gates (ADR 0002 risk 1).** `quality` and `security` never closed, nor `release` (check
3 is no close test). The engine marks them `Unproven`: they still run and their red still blocks,
so nothing is looser than today, but their green is recorded `proven: false` and is no key; every
approval request and `tminus status` name them, until their close tests pass (`gates-close`). A
gate whose lock changes must close again before any task runs on it: the rules approval (section
4), or prepare for a tightening, runs its close test first and shows the result; one that no
longer closes refuses every task until its lock is fixed. Only gates that never closed before the
engine run unproven.

### 3. Levels by risk

**Risk.** Color as ADR 0002 section 5 says (`classify.rs:84-104`); a new lock when a touched or
planned item matches the lock's patterns (`rules.rs:145-147`). Computed at the Lock phase and again
at each run from what was touched: if it rises, the levels rise with it.

**Today's table as data.** `gdd/rules.toml:18-48` and `default_table` (`rules.rs:309-342`), with
what the code makes of each key:

```toml
[level.green]
fast = { level = "lock" }                                                                   # 20-24
full = { level = "lock+guard", new_lock = "lock+guard+approval", lock_step = "new_lock" }   # 26-30
[level.yellow]
fast = { level = "lock+guard", lock_step = "always", run = ["mutation"] }                    # 32-36
full = { level = "lock+guard", new_lock = "lock+guard+approval", lock_step = "always", run = ["mutation"] }  # 38-42
[level.red]
full = { level = "lock+guard+approval", lock_step = "always", run = ["mutation"] }           # 44-48
```

| Old key | New | Why (the code today) |
|---|---|---|
| `reviewer = true` / `false` | `level = "lock+guard"` / `"lock"` | the Reviewer runs or not (`classify.rs:129`) |
| `g4_if_g1 = true` in a full lane | `new_lock = "lock+guard+approval"` | G4 with new scenarios (`classify.rs:131`) |
| `g4_if_g1` in a fast lane | dropped | no effect (`classify.rs:115`) |
| `tests_first = true` | `lock_step = "always"`; green: `"new_lock"` | `&& (g1 \|\| color != Green)` (`classify.rs:127`) |
| `tests_first` unset / `false` | `"new_lock"` / `"never"` | `unwrap_or(g1)` (`classify.rs:127`) |
| `mutation = true` | `run = ["mutation"]` | runs `mutation*` (`gates/mod.rs:752`) |
| `review` | `guard` | unset: quick (`rules.rs:91-101`) |

A level applies to every gate unless a line names one (`[level.red.release]`). **Deviations from
ADR 0002 section 3:** `release` takes the task's level, not `lock+guard+approval`, which would ask
the human for every change (today a change without G4 is promoted, `recipe/mod.rs:18-19`); and
`tests_first` and `mutation` move from `[software]` into the level rule, as the floor and the
loosening check must see them and the core reads no blueprint section (ADR 0002 risk 2). The
ladder has no approval without the guard: an old `reviewer = false` with `g4_if_g1 = true` reads
as `lock+guard+approval` for a new lock, stricter than today.

**One guard, one sitting.** Levels are per gate, but a task has one guard and one approval sitting,
as today: the guard runs if any gate asks for it, at the deepest depth asked; the sitting covers
every gate at the approval level and names which ones asked.

**The floor.** Code in `level.rs`, applied after the rules and checked when they are read: a rules
file that writes less is refused, naming the key (old keys keep their names and the message of
`config.rs:91-95`, which `manifesto-core`'s steps check, `acceptance.rs:3446-3462`).

| In | The core imposes | Today |
|---|---|---|
| red | every gate `lock+guard+approval`, guard `deep`, every optional command runs | `CORE`, `rule` (`rules.rs:227-244, 282`) |
| red | the human approves the plan | `classify.rs:132` |
| any color | a new lock is sealed only by the human | `prepare.rs:103-105`, `approve.rs:399-401` |
| any color | the core's own items (`tminus/**`, `.tminus/**`) are red and protected for every agent role | new: today `zones.toml:13-16` and `rules.toml:52` decide, and a project may drop them |

The floor binds what reaches the integration target. A spike brings only its ADR
(`recipe/mod.rs:21-27`), so its own rules (`classify.rs:140-153`) stay the software blueprint's
spike extra, and its approval is always on.

**What of the lane survives.** In the core, as an input of the rule: `Lane::Fast` from the risk
(below red, no new lock) and the plan's `resumable` (`classify.rs:115`); never from the blueprint.
`resume_green` is no lane: it is a key that still holds (section 1).

### 4. Gates only get stricter

**Loosening, precisely.** The engine compares two versions of the effective rules (after reading
old names, defaults and the floor), not their text: comments, reasons and order are not changes.
**A change is a tightening only where the engine can prove it; anything else is a loosening.**

| Part | Loosening | Tightening (no approval) |
|---|---|---|
| Gates, stages | a gate or a stage removed; a named check taken from a stage | added |
| Levels | in any of the 3 x 2 x 2 cells (color, lane, new lock) of any gate: a lower level, a guard less deep, a lock step less often, an optional command no longer run | the reverse |
| Close tests | removed or changed | added |
| Areas | the default lowered; an old (pattern, color) with no new pattern equal to it at that color or higher; a new pattern below the new default | widened, raised, or added at or above the default |
| Protected, lock items | a pattern removed for any role; a lock pattern removed or a skip pattern added | a pattern added |
| Limits | attempts, replans, guard fixes, spike budget or command time raised | lowered |
| Stock locks | a command removed or renamed, its text changed, moved to another stage, or now optional | a command added, or an empty one filled |
| Known debt | an entry added, a count raised or dropped | paid or lowered |
| Anything else read to decide gates | any change | |

A changed command counts as loosening even when it is stricter, as the engine cannot order two
shell lines. Measured cost: 9 of the 10 command edits since the Setup would have asked (Context).

**The area rule is sound.** An item's color is the highest of the patterns matching it, or the
default (`zones.rs:139-150`). An item matched before keeps a pattern at least as high; one matched
by no old pattern had the old default and gets the new one, or a new pattern not below it. So no
item goes down. It may flag a change that lowers nothing (a pattern covered by a wider one).

**Detection.** `loosened(old: &Rules, new: &Rules) -> Vec<Loosened>`, a pure function. `old` is
what `.tminus/rules.json` approved, kept whole as `zones.json` keeps its red tables
(`approve.rs:367-372`), or for a task in flight the rules sealed at its Lock phase (`rules_at`).
The hash: SHA-256 of the effective rules as JSON with sorted keys, as `red_hash` (`approve.rs:713-720`).

**What the approval records.** A `RulesApproval` (section 1), by `tminus approve rules --by human`
(today `gdd gate`). Agents gdd starts are refused before anything is read (`lib.rs:860-863`,
`approve.rs:289-294`), and so is any `--by` but `human` (`approve.rs:399-401`). A tightening is
recorded `by: "machines"` without asking.

**Where it is enforced**, besides the floor when the rules are read; not in CI, which has no
state (`ci.rs:1-3`) and keeps failing harness paths inside a change branch (`ci.rs:113-119`):

1. **At prepare, every task**, as the red zones are (`prepare.rs:92-96`): main's effective rules
   against the approved ones. Same: go. Stricter: recorded by the machines, go. Looser: refused,
   listing what loosened and how to approve it. No task starts on rules nobody approved.
2. **At integrate:** a task that changes the rules touches the core's items, so it is red and its
   level includes approval; its approval request lists what it loosens, and the human's OK
   records the `RulesApproval` (main's hash to the task's) with the result, in one sitting.
3. **In flight:** a task is judged by the rules sealed at its Lock phase, or by main's when they
   are stricter in every part (`loosened(sealed, main)` is empty). Loosening later frees no task
   already locked, as for protected paths (`guard.rs:199-203`). Today's reading from the branch
   (`lib.rs:1378-1391`) goes: main's rules, as for the zone map.

**The ratchet.** Known debt is part of the compared rules. Paying and lowering it at integrate
(`debt.rs:402-441`) are the engine's own tightenings, recorded by the machines; adding a debt or
raising a count is a loosening. `gdd adopt` writes the first list under the first rules approval.
`NEVER_DEBT` and `can_owe` (`debt.rs:54, 285-287`) stay the blueprint's.

### 5. The Gates and Lock phases in the engine

**Gates phase** (replaces G0, `approve.rs:298-327`). The engine closes it, never a person or an
agent (today only `--by machines`, `approve.rs:306`), when: the Blueprint phase is answered (today
the Project Brief, `approve.rs:312`); the human approved the areas and the first rules; and every
gate is valid. A project coming from G0 keeps it closed (section 6). A changed lock reopens only
its gate (section 2).

**Lock phase** (replaces the plan and G1, `approve.rs:392-480`, `prepare.rs:87-167`). Per task,
before any agent: risk and levels are decided; the human approves the plan where the floor asks;
the lock items the task brings pass the lock's check (phrasing) and the human seals them with
their SHA-256; new: `Levels.rules`, the hash of the rules it is judged by, is sealed with them.
The launch checks the seal before anything else (`prepare.rs:117-120`, `approval-holds.feature`).

**The approval level** (replaces G4). Where G4 is today: after the gates before integrating, the
guard, integrating and pre, before promotion (`recipe/mod.rs:15-19`), the first step that cannot
be undone. The request names each gate's key (proven or not), the guard's verdict, what the task
loosens and its lane. The answers (`recipe/mod.rs:756-783`): **OK** records `approvals.result` with
the mark the human saw (new: G4 keeps no hash today, only G1 does, `approve.rs:409-422`) and
promotes. **KO** rejects the task, production untouched (`765-772`). **Polish** runs the recipe
again from the cross agent, the notes in its first round, until it asks again (`773-782`,
`991-995`); the mark moved, so the earlier approval no longer holds.

### 6. Migration

Read old as aliases, always (ADR 0002 risk 4); write new from `rename-tminus` (ADR 0002 step 9).

| Today | Read as |
|---|---|
| `[color.<c>.<lane>]` | the level rules of section 3 |
| `[limits] builder_retries`, `replans`, `reviewer_fixes`; `[spike] max_usd`; `command_seconds` | `limits`: attempts, replans, guard fixes, spike budget, command time |
| `[acceptance]` | the `works` lock: items, skip; `required` its scenario count |
| `[protected] builder_test`, `builder_build`, `reviewer` | `lock`, `cross`, `guard` (ADR 0002 section 3) |
| `[[zone]] paths` | `[[area]] items` |
| `[g2]`, `[g3]` | stages `every_round`, `before_integrating`; each command to a gate by the kind before its first `-` (`debt.rs:41-50`): `unit`, `acceptance` to `works`; `lint`, `types`, `architecture`, `quality`, `mutation` to `quality`; `security`, `dependencies`, `lockfile`, `secrets` to `security`; any other to `works`, named in the report |
| `[release]`, `[selftest]` | the `release` gate's stages; `[close_test.works]` |
| `g0.json` + `selftest.json` (4/4) | Gates phase closed; `works` valid; `quality`, `security`, `release` unproven |
| `zones.json` | the areas approval, same hash and formula, red tables only (ADR 0002 risk 3) |
| (none) | the rules baseline: today's effective rules, `by: "migration"` |
| `gates.G1` + `approved`; `gates.plan`; `gates.G4` | `approvals.lock` (same seal); `approvals.plan`; `approvals.result` without a mark |
| `gates.G2`, `gates.G3`, `g2.json`, `g3.json` | keys of `every_round`, `before_integrating`; `proven` from the gate's validity |
| `decision` (`g1`, `g4`, `reviewer`, `mutation`, `tests_first`, `review`) | the task's sealed levels |
| `gate: "G2"`, `phase: "2-build"` in the log | read by the reports as they are |

**No approval stops counting.** G0 counts as the Gates phase closed; unproven gates block nothing
that does not block today. The red-zone hash and the G1 seals keep their bytes and formula (ADR
0002 section 5). A task paused at G4 stays paused; its OK holds for the mark its worktree has when
the engine first reads it, as no agent runs while a task waits.

**None counts for more than was seen.** The baseline is what today's code does with the old files,
not what they say: `reviewer_fixes = 3` is unread today, so the baseline is 1 and the 3 asks as a
loosening. The baseline is no approval (risk 7). `zones.json` covers only the red tables it hashed;
an old G4 without a mark approves nothing new; no gate becomes valid by migration.

### 7. Scenarios to guard it

By the Change of section 8 that adds them:

- **Invariant 6**, for `manifesto-core.feature` (6b): Loosening a gate waits for the human's
  approval · An agent cannot approve a loosening · Tightening a gate needs no approval
- **Data** (5a): Old records and old rules files are read as before · A command no gate claims
  goes to the gate that checks the work
- **Validity** (5b): A gate counts as valid only once a planted failure turned it red · A gate
  that stays green with its planted failure is not valid · A gate whose lock changed after it
  closed must close again before the next task · An unproven gate still blocks when it is red,
  and the approval request names it · A close test never touches the place where a task works ·
  A gate without a lock, or with a lock an agent may write, is refused when the rules are read
- **Levels** (6a): Each gate gets the level its rules give the risk of the task · In the most
  sensitive area every gate asks for the guard and the human's approval · The rules that decide
  risk are always in the most sensitive area · The fast lane needs a resumable task without a new
  lock, outside the most sensitive area
- **Stricter** (6b): A changed command counts as loosening · Raising a limit of attempts counts as
  loosening · Lowering the color of an area counts as loosening · Adding to the known debt counts
  as loosening, and paying it does not · No task starts on rules that loosened since the human
  approved them · The Lock phase seals the rules the task is judged by · A task keeps the rules its
  Lock phase sealed when main's get looser · A task that loosens the rules shows it in its
  approval request, and one approval covers both
- **Phases** (6c): The Gates phase closes once every gate was seen to close · The approval holds
  only for the work the human saw · Polish runs the work again with the human's notes and asks
  again

### 8. Changes

ADR 0002 steps 5 and 6 split in five, in this order; each is red (gate integrity) and leaves
main usable. Steps 1 to 4 and 7 to 9 stay as they are.

| # | Change | Type | What it does | Scenarios that guard it |
|---|---|---|---|---|
| 5a | `gates-as-data` | Refactor | `GateDecl`, `GateId`, stages, the commands grouped by gate, `Key` for the reports; old names read as aliases | `gates`, `gate-details`, `gate-hygiene`, `mutation-speed`, `security-exceptions`, `fix-rounds`, `ratchet`, `spike-flow`, + Data |
| 5b | `gates-close` | Feature | `Validity`, close tests per gate in data, `plant`, unproven gates; close tests for `quality`, `security` and `release` | `selftest-debt`, `gates`, + Validity |
| 6a | `levels-by-risk` | Refactor | `Risk`, `LevelRule`, `Levels`, the floor as code; the color table read as levels; the core's items red and protected | `core-levels`, `core-levels-init`, `classify`, `cli-classify`, `review-depth`, `manifesto-core`, + Levels |
| 6b | `only-stricter` | Feature | `loosened`, `.tminus/rules.json`, `tminus approve rules`, the checks at prepare and integrate, rules sealed per task and read from main | `manifesto-core` (its three new scenarios), `ratchet`, `approvals`, `who-approves`, + Stricter |
| 6c | `phases-and-levels` | Feature | G0 as the Gates phase, G1 as the Lock phase, G4 as the approval level, in records and requests, with aliases; the result approval keeps its mark | `approvals`, `approval-holds`, `g4-explainer`, `sitting`, `sitting-g4`, `steps-reuse`, `entry-zones`, `who-approves`, + Phases |

5b comes before 6b so the loosening check can compare close tests; 6b needs 6a's model to compare
levels. 6c renames last, as most words live there (risk 5).

### 9. Risks and open questions

1. **Who is the human.** `None` is "a person, or the Orchestrator working with them"
   (`approve.rs:286-288`): an interactive agent can type `--by human`. Recommendation: the rules
   approval reads a short code it printed from the terminal and refuses without one; later, every
   human approval. It proves no person, but stops a tool call without a terminal (`lib.rs:1570`).
2. **Approvals often, for little.** 14 of 22 harness commits would have asked; 4 loosened in
   effect. Recommendation: accept it; each request shows the exact lines and how many items a
   removed pattern matches today. Order no command text unless `tminus learn` shows rubber stamps.
3. **Three gates unproven after migration.** Recommendation: `gates-close` right after
   `gates-as-data`; `release` needs a per-project plant (a command that breaks pre), which the
   human writes at the Setup, as `[selftest]` today.
4. **Rules read from main, not the branch.** A human's edit on a change branch stops judging that
   branch. Recommendation: accept; the zone map works this way (`lib.rs:2353-2359`).
5. **49 of 80 approved feature files name G0-G4, on 382 lines.** Rewriting them needs the human's
   approval of each. Recommendation: 6c keeps the old names as aliases in the CLI and messages;
   approved scenarios stay as they are, and only new ones use the method's words.
6. **The approval lives in one clone's state**, so CI does not enforce it and a second person is
   asked again. Recommendation: keep it so (an approval holds for who saw it); move it into the
   repository when a project has a second person.
7. **The baseline was never approved.** Recommendation: record it `by: "migration"` and ask once
   in the Decision Inbox, without blocking tasks.
8. **The fast lane lowers levels on a declaration** (`resumable`), not on what was touched.
   Recommendation: keep it (principle 5), never in red or with a new lock, and name it in the
   approval request.

### Summary

Today the tool decides how strictly to check each change from settings anyone at the keyboard can
edit, and nothing notices when an edit weakens the checks: in eight days it happened four times,
unrecorded. This decision makes the checks a small engine with clear rules. A check counts only
after the tool has seen it fail on purpose; three of today's never were, so they keep blocking bad
work but no longer count as proof. How strict each check is follows from how sensitive the touched
area is, with a minimum for the most sensitive areas that no settings can lower. Settings may only
get stricter on their own: the tool compares each new version with the last approved one, and any
weakening, even a doubtful one, stops all work until the human approves it, in a record of who,
when and what. Five small changes build it, and the tool keeps working after each.

## Alternatives

- **Compare the rules as text.** Rejected: a comment would ask for an approval, and adding a key
  can loosen (`7c7000f` added `max_usd = 10` above the default 5).
- **Let the blueprint say whether a command change is stricter.** Rejected: the core would trust
  blueprint code for an invariant (ADR 0002 risk 2).
- **Count a changed command as stricter when the old text is kept and `&&` more follows.** Not
  now: none of the 9 edits had that shape (risk 2).
- **Block every task while a gate is unproven.** Rejected: it would stop every project today for
  gates that block exactly as before; the record says what they prove.
- **Enforce in CI on main.** Rejected for now: CI has no state, so the approval would need a second
  record in the repository that can drift from the first (risk 6).

## Consequences

- ADR 0002 steps 5 and 6 become the five Changes of section 8. `Blueprint` gains `plant`; `selftest.rs` loses its setting aside and giving back.
- Every change is judged by main's rules, sealed at its Lock phase, no longer by its branch's.
- The human approves rules that loosen, once per version; harness edits on main are no longer
  free. `manifesto-core.feature` gets the invariant 6 scenarios, with the human's approval.
