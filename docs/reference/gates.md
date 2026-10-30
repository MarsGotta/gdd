# Gates

What each gate of GDD checks, when it runs, who decides, its limits and what parks a change,
by color. The values are those of the `gdd/rules.toml` that `gdd init` writes; see
[Configuration](configuration.md#rulestoml) for every key.

## Gates by color

A change's color comes from the paths it plans and touches (see
[zones.toml](configuration.md#zonestoml)); its lane from its color, its scenarios and whether
it is resumable (see [Lanes](#lanes)). Read each column as one kind of change.

| | Green, fast | Green, full | Yellow, fast | Yellow, full | Red |
|---|---|---|---|---|---|
| G0 Setup Gate (once per project) | before any change | before any change | before any change | before any change | before any change |
| Red zones approved | before any change but a spike | same | same | same | same |
| G1 Scenario Gate | never: no new scenarios | when scenarios are new or changed | never | when scenarios are new or changed | when scenarios are new or changed |
| Plan approved by the human | no | no | no | no | always |
| Test mode before build mode | no | only with G1 | yes | yes | yes |
| G2 Loop Gate | every round | every round | every round | every round | every round |
| G3 Merge Gate | yes | yes | yes | yes | yes |
| Mutation testing in G2 and G3 | no | no | no (`mutation = true` turns it on) | no (`mutation = true` turns it on) | always |
| Reviewer | no | quick | quick | quick | deep, security included |
| G4 Release Gate by the human | no | only with G1 | no | only with G1 | always |
| Human stops | 0 | 0 to 2 | 0 | 0 to 2 | 2: the sitting, then G4 |

Red is never in the fast lane. A spike follows a recipe of its own: see [Spikes](#spikes).

The core of GDD is always on for red: mutation testing, the Reviewer (deep) and the human G4. A
rules file that switches any of them off for red is refused.

## Lanes

| Lane | When | What it changes |
|---|---|---|
| Fast Lane | The color is not red, the change creates or changes no `.feature` that needs G1, and it was created with `gdd new-change --resumable` (another agent could pick it up from its ticket alone). | Its rule is `[color.<color>.fast]`. With the tables of `gdd init`: no G1, no human G4, zero human stops; green also goes without test mode and without the Reviewer. |
| Full Lane | Every other change. | Its rule is `[color.<color>.full]`. |

Color beats work type and lane: a red change always takes the full lane and the core of GDD.
The Orchestrator judges whether a change is resumable when it creates it; `gdd classify` prints
the lane and why.

## G0 Setup Gate

**When:** Once per project, at the end of Setup (phase 0), before any change can be prepared.

**What runs:** `gdd selftest`, the harness self-test, four checks:

1. A failing test turns G2 red: with the file of `[selftest] file` set aside, every G2
   command passes, or is held to its known debt in `gdd/debt.toml` as G2 holds it; with the
   failing test of `[selftest] content` planted there, one fails. A `gdd/debt.toml` gdd cannot
   read fails the check, with the reason.
2. Editing a protected file is blocked: `[protected] all` covers `gdd/rules.toml` and
   `.github/workflows/gdd.yml`, and that workflow runs `bin/gdd ci`.
3. Rollback works in pre: `[release] rollback_pre` passes, or the release is simulated.
4. Zero scenarios counts as a failure: G3 counts none in an empty folder, and
   `[acceptance] required` is not `false`.

The result goes to `.gdd/selftest.json` and the log. `gdd selftest` exits with 1 unless all
four pass (`harness 4/4`).

**Who decides:** The machines: `gdd gate G0 ok --by machines`. It opens only when
`docs/brief.md` exists and the last self-test passed; a person cannot open it (`--by human` is
refused).

**Limits:** None. Fix the harness and run `gdd selftest` again.

**What it blocks:** `gdd prepare` refuses every change while G0 is pending.

At G0 there is no zone map beyond what `gdd init` wrote: the harness and the supply chain are
red, and unmapped code is yellow.

### When it says no

Real messages, the most common first.

You run `gdd prepare` before G0 is open:

```text
error: G0 pending: run the Setup (the gdd-setup command).
```

Main already fails a G2 command (here lint, on an unused import), so the self-test cannot
see a failing test turn G2 red. Fix it, or record it as known debt with `gdd adopt`:

```text
KO a failing test turns G2 red (G2 is red without it: lint)
OK editing a protected file is blocked (Path Guard and CI)
OK rollback works in pre
OK zero scenarios counts as a failure
harness 3/4
```

You try to open G0 while the last self-test is not 4/4:

```text
error: G0 cannot open: the harness self-test is not 4/4 (bin/gdd selftest).
```

The Brief is missing:

```text
error: G0 cannot open: docs/brief.md (Project Brief) is missing.
```

## G1 Scenario Gate

**When:** At the sitting of a change, when the change creates or changes a `.feature` file
inside `[acceptance] dir` and outside `[acceptance] regression`. Changing only regression
scenarios needs no G1.

**What runs:** Nothing: the human reads the scenarios (`gdd sitting <id>` shows them) and
approves or asks for changes. Approving G1 seals the scenario files of the change as its branch
has them, with their SHA-256, in the record, hidden examples included.

**Who decides:** The human: `gdd gate G1 ok <id> --note "..."`, or `gdd gate G1 ko <id>` to ask
for changes.

**Limits:** G1 refuses hidden examples (`<dir>/hidden/`) whose step sentences no visible
scenario has: test mode never sees them and could not write their steps. One sitting per change
covers G1, the red zones and the plan.

**What it blocks:** `gdd prepare` refuses a change with new or changed scenarios without G1.
After G1, any change to a sealed file fails Path Guard for every role (`scenario approved in G1
and modified afterwards`).

### Red zones

The red zones of `gdd/zones.toml` need a human approval, once and again whenever any red zone
changes: `gdd gate zones ok --note "..."` (only `--by human`). The approval keeps the red zones
and their hash in `.gdd/zones.json`; approving them as they already are writes nothing. Until
they are approved, only a spike can be prepared. `gdd status` says `pending` or `changed`.

### The plan of a red change

A red change needs its plan approved by the human before it is prepared: the Orchestrator
writes `.gdd/changes/<id>/plan.md` (what, how, risks, how to roll back), the human reads it at
the same sitting, and `gdd gate plan ok <id>` records it.

### When it says no

Real messages, the most common first.

The change has new scenarios that nobody approved yet, so `gdd prepare` refuses it:

```text
error: New or changed scenarios without G1: a human must approve them (the gdd-change command).
```

You read the scenarios and ask for changes (`gdd gate G1 ko cart-total`):

```text
G1 ko on cart-total (human).
```

The red zones of the project are not approved yet, usually on its first change:

```text
error: Red zones not approved yet: the Orchestrator proposes them at the end of Discover. (bin/gdd gate zones ok --by human)
```

A hidden example uses a sentence that no visible scenario has:

```text
error: G1 refused: hidden examples use step phrasings that no visible scenario has, so test mode can never write them. Reuse visible sentences with new data:
  - an empty cart
```

## G2 Loop Gate

**When:** After every round of build mode; whenever a Builder runs `bin/gdd g2` (test mode
runs it to see its new tests fail for the right reason); and on a relaunch, before any Builder
runs.

**What runs, in order:**

1. Path Guard for the role, on what the change touched since its build started. When it
   fails, no command runs: a role that touched the gate configuration could make the commands
   its own.
2. Each `[g2]` command of `gdd/commands.toml`, in the alphabetical order of their names, each
   within `[limits] command_seconds`. `mutation*` commands run only when the change needs
   mutation testing (red, or a rule with `mutation = true`).
3. Path Guard again.
4. In build mode, the scope check: a path outside the planned paths whose color is the
   change's or lower is accepted and listed in the Decision Inbox; a higher one fails.

The report goes to `.gdd/changes/<id>/g2.json`, and to the log (one line per command, one for
Path Guard and one for the gate) unless an agent ran it.

**Who decides:** The machines. It passes when nothing failed. A command with known debt in
`gdd/debt.toml` that fails only warns, unless it reports more than its count.

**Limits:** `[limits] builder_retries` rounds of build mode (3), at least one. Still red:
`[limits] replans` replans (1), each an Orchestrator call that writes `replan.md`, a round of
test mode when the replan ends with `NEXT: test`, one more round of build mode and G2. Only the
Orchestrator widens the scope.

**What parks:** G2 still red after the rounds and the replans (`G2 still red after 3 retries
and 1 replan`). Path Guard failing after a round of test mode (`Path Guard failed after test
mode: ...`).

A relaunch whose code already passes G2 skips the Builder (`resume_green` in the decision).

### When it says no

Real messages, the most common first.

A G2 command fails, here lint on an unused import. `gdd g2` prints its report as JSON:

```text
{"ok":false,"gate":"G2","color":"yellow","passed":["acceptance","architecture","types","unit","path-guard","scope"],"skipped":[],"failures":[{"name":"lint","ok":false,"code":1,"duration_s":0.0,"tail":"F401 [*] `os` imported but unused\n --> src/shop/cart.py:1:8\n ...\nFound 1 error.\n[*] 1 fixable with `--fix`.\n"}],"out_of_scope":[]}
```

The Builder used its rounds and the replan, and G2 is still red. The change parks; its
Decision Inbox says:

```text
## Parked
- G2 still red after 3 retries and 1 replan
```

The Builder in build mode edits a scenario approved at G1. Path Guard fails and no command
runs:

```text
{"ok":false,"gate":"G2","color":"yellow","passed":["scope"],"skipped":[],"failures":[{"name":"path-guard","tail":"features/cart_total.feature: protected path for builder-build\nfeatures/cart_total.feature: scenario approved in G1 and modified afterwards\nThe gate commands did not run: Path Guard must pass first."}],"out_of_scope":[]}
```

The Builder touches a red path the plan does not list:

```text
{"ok":false,"gate":"G2","color":"red","passed":["acceptance","architecture","lint","types","unit","path-guard"],"skipped":[],"failures":[{"name":"scope","tail":"src/shop/payments/charge.py: outside the package and red (the change is yellow); remove it, or the Orchestrator widens the scope"}],"out_of_scope":[]}
```

## G3 Merge Gate

**When:** Once G2 is green, and again on top of the latest main when main moved while the
change was built.

**What runs, in order:**

1. Path Guard, then each `[g3]` command (alphabetical, within `command_seconds`, `mutation*`
   only when needed), then Path Guard again, and the scope check in build mode, as in G2.
   The stack sheets put here the full scenario run (hidden examples included), the lock file,
   static security analysis, dependency licenses, secrets on the diff and mutation testing on
   the diff.
2. The scenario count: zero scenarios in `[acceptance] dir` fail it when `[acceptance]
   required` is `true` or the change has scenario files of its own.
3. The security exceptions (`[[IgnoredVulns]]` of `osv-scanner.toml`, `[advisories] ignore` of
   `deny.toml`): one without a reason fails it; the ones the change adds are listed in the
   Decision Inbox and at G4.
4. When the change has a Reviewer: the review, at the depth of its rule. The Reviewer answers
   `VERDICT: OK` or `VERDICT: KO` as the last line of its review.

The report goes to `.gdd/changes/<id>/g3.json` and the log.

**Who decides:** The machines and the Reviewer. When both pass, gdd records G3 `ok` by
`machines`, then merges.

**Limits:** Red G3: one fix by the Builder (`fix=g3`), G2, and G3 again, which decides.
Reviewer KO: one fix (`fix=review`), G2, G3 and a second review that checks only the reasons of
the first. Merges go one at a time; a change whose main moved takes the latest main and passes
G3 again.

**What parks:** G3 still red after its fix (`G3 still red after one fix`). A KO, or a red G3,
after the Reviewer's fix (`Reviewer KO or G3 red after one fix`). A conflict with the latest
main (`main moved under it and the merge has conflicts in ...`). G3 red on top of the latest
main.

After the merge, the merge commit carries the change's record (see
[Files](files.md#the-merge-commit)), and the `build` and `deploy_pre` steps take it to pre.

### When it says no

Real messages, the most common first.

A G3 command fails, here a dependency with known vulnerabilities (`jinja2==2.10` in
`uv.lock`):

```text
{"ok":false,"gate":"G3","color":"red",...,"failures":[...,{"name":"security","ok":false,"code":1,"duration_s":1.0,"tail":"...Total 1 package affected by 6 known vulnerabilities (0 Critical, 2 High, 4 Medium, 0 Low, 0 Unknown) from 1 ecosystem.\n6 vulnerabilities can be fixed.\n...| https://osv.dev/PYSEC-2019-217      | 8.6  | PyPI      | jinja2  | 2.10    | 2.10.1        | uv.lock |\n..."}],"out_of_scope":[],...}
```

The Reviewer answered `VERDICT: KO`, the Builder fixed it once, and the second review
still said KO. The change parks:

```text
## Parked
- Reviewer KO or G3 red after one fix
```

G3 is still red after the Builder's one fix:

```text
## Parked
- G3 still red after one fix
```

Main moved while the change was built, and the merge has conflicts:

```text
## Parked
- main moved under it and the merge has conflicts in crates/gdd/src/lib.rs: merge main into change/gdd-upgrade in its worktree (/home/me/gdd/../gdd.worktrees/gdd-upgrade), settle the conflicts, commit, then gdd launch gdd-upgrade
```

## G4 Release Gate

**When:** Once the change is merged and in pre.

**What runs:** `tests_pre`, then `demo` (`gdd release pre`); the change is `in-pre` whatever
they say. The human G4 applies when the change is red, when it had G1 and its rule has
`g4_if_g1 = true`, and always for a spike: the recipe pauses (`.gdd/changes/<id>/run.json`)
with the Decision Inbox and the demo for the human. Otherwise the machines record G4 `ok` and
promote the change at once.

**Who decides:** The human, with `gdd resume <id> --g4 ok|ko|polish`:

| Answer | What happens |
|---|---|
| `ok` | `promote` (the same artifact, to production), then `observe`. The change is `closed`. |
| `ko` | The change is `rejected`; production is untouched; its worktree goes. |
| `polish --note "..."` | One more pass of the Builder with the human's notes (build mode, `fix=polish`, with its rounds and replan), then G3, the Reviewer, the merge when there is something new, pre, and G4 again. It replaces a KO; it is not an extra stop. |

**Limits:** A promotion that fails leaves production untouched and the change waiting at G4:
the same `ok` retries it. In `[versions] mode = "auto"`, a promoted change with a changelog
line gets its version once the promotion passed.

**What parks:** A change caught in the middle of a promotion (`promoting`) parks with a message
to look at production first, so nothing is deployed twice. When `observe` fails, `rollback`
runs and the change goes to `rolled-back` with a request for a hotfix.

### When it says no

Real messages, the most common first.

You answer polish without saying what to polish:

```text
error: polish needs your notes: gdd resume cart-total --g4 polish --note ...
```

You answer a change that is not waiting at G4:

```text
error: Change cart-total is not paused at G4.
```

Someone tries to promote a change that needs a human G4 before the human answered:

```text
error: This change needs a human G4 before promotion.
```

Production gets worse after the promotion: `observe` fails (here it prints
`error rate up`), gdd rolls back and asks for a hotfix:

```text
error: release.observe failed (exit code 1):
error rate up
--- end of output ---
error rate up
Rollback: ok
Production failed while observing: rolled back to the previous version. A hotfix is needed. Message in .gdd/changes/cart-discount/inbox.md
```

## Learn

Learn (phase 5) has no gate: it only hardens.

- `gdd learn` runs the `[learn]` tools of `gdd/commands.toml` once (duplication, unused code),
  keeps what each said in `.gdd/learn/`, lists the polish notes the human gave at G4 and the
  classes of finding that repeat in the Reviewer's KO reviews of two changes or more. It exits
  with 0 whatever the tools found.
- The Orchestrator (`gdd-learn` command) turns each repeated error into one rule, lint, hook,
  test or line of `docs/standards.md`, and never loosens a gate. A proposal that changes
  `gdd/` is red: the human decides.

## Spikes

A change of type `Spike` answers one question and its code never reaches main.

| Step | What happens |
|---|---|
| Prepare | Only G0 is needed; the red zones may still be pending. No G1, no plan, no test mode, no mutation testing, no Reviewer. Its color still picks its Builder. |
| Build | Rounds of build mode (`mode=spike`) within `builder_retries`, then the replans, never test mode. |
| G2 | Path Guard and an ADR: the branch adds or changes `docs/adr/*.md` with a `## Decision` line. No command of `gdd/commands.toml` runs. |
| Budget | After each agent call: the spike parks once its calls cost `[spike] max_usd` (5 USD). |
| G3, merge, pre | None. |
| G4 | Always the human. OK brings only the ADR to main; the spike's branch stays. |

A spike with variants (`--option` once per option, or `--variants <n>`, 2 to 4) builds each
variant on its own branch `change/<id>-v<n>`, one after another, each with the whole budget. It
pauses at G4 when at least one variant passed its gate, and parks when none did (`no variant
passed its gate`); `gdd resume <id> --g4 ok --variant <n>` brings that variant's ADR.

## What parks a change

A parked change stops with a message in its Decision Inbox and keeps its worktree; relaunch it
with `gdd launch <id>` once the cause is fixed.

| Cause | Where |
|---|---|
| G2 still red after the rounds and the replans | [G2](#g2-loop-gate) |
| Path Guard failed after test mode | [G2](#g2-loop-gate) |
| G3 still red after one fix; Reviewer KO or G3 red after one fix | [G3](#g3-merge-gate) |
| Merge conflict with the latest main, or G3 red on top of it | [G3](#g3-merge-gate) |
| The main checkout cannot take the merge: not on the main branch, or uncommitted changes in tracked files | [G3](#g3-merge-gate) |
| An agent failed, or did not answer within `[limits] agent_timeout_s` | any agent call |
| An agent changed GDD state, or the Reviewer changed a file | any agent call |
| A spike reached its budget, or no variant passed its gate | [Spikes](#spikes) |
| Caught in the middle of a promotion | [G4](#g4-release-gate) |
| `gdd park <id> <reason>` | by hand |

## See also

- [How risk picks the gates](../explanation/risk.md): why color wins and what each color costs.
- [Configuration](configuration.md#rulestoml): the keys behind every row of these tables.
- [Roles](roles.md): who runs each step and what each may touch.
- [Answer the Release Gate](../how-to/release-gate.md): OK, KO or polish, step by step.
