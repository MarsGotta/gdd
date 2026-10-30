# Troubleshooting

Each entry starts from what you see, a message of gdd or a change that stops, then gives the
cause and the fix. Messages are quoted as gdd prints them; `CHANGE` stands for the id of your
change, and the numbers, paths and ids in them are examples. For a change that parked, start with
[Get a parked change going again](parked-change.md).

## Before you start

- Run `bin/gdd status` first: it says where G0, the red zones, the self-test and every change
  stand.
- For a change, `bin/gdd inbox CHANGE` gives its Decision Inbox, and `.gdd/changes/CHANGE/`
  holds the gate reports (`g2.json`, `g3.json`) and what each agent said (`outputs/`).

## Find your message

| When | Entries |
|---|---|
| Before the build starts | [G0 pending](#g0-pending-run-the-setup) · [G0 cannot open](#g0-cannot-open) · [Self-test: G2 red](#self-test-g2-is-red-without-the-failing-test) · [Red zones not approved](#red-zones-not-approved-yet) · [Red zones changed](#red-zones-changed-after-their-approval) · [Scenarios without G1](#new-or-changed-scenarios-without-g1) · [Red plan](#red-change-a-human-must-approve-the-plan) · [Same model family](#reviewer-and-builder-are-the-same-model-family) · [Agent not found](#cannot-find-the-program-of-an-agent-on-path) · [Uncommitted changes](#the-working-tree-has-uncommitted-changes) · [Not the main checkout](#run-gdd-launch-from-the-main-checkout) · [The cap](#the-cap-of-changes-building-is-reached) |
| At a sitting | [G1 refused](#g1-refused-hidden-examples-use-step-phrasings) · [No terminal](#gdd-sitting-needs-a-terminal) · [Builder cannot record](#a-builder-or-reviewer-cannot-open-or-close-gates) |
| During the build | [G2 still red](#parked-g2-still-red-after-3-retries-and-1-replan) · [Protected path](#protected-path-for-builder-build) · [Commands did not run](#the-gate-commands-did-not-run-path-guard-must-pass-first) · [Approved scenario modified](#scenario-approved-in-g1-and-modified-afterwards) · [Outside the package](#outside-the-package-and-red) · [Test mode wrote code](#path-guard-failed-after-test-mode) · [Agent changed GDD state](#changed-gdd-state) · [Agent did not answer](#did-not-answer-within-n-seconds) · [Spike budget](#the-spike-spent-its-budget) |
| In G2 and G3 | [Command timed out](#timed-out-raise-limitscommand_seconds) · [Mutation timeouts](#mutation-testing-times-out) · [Not configured](#gddcommandstoml-has-no-commands-for-this-gate) · [Zero scenarios](#zero-scenarios-counts-as-a-failure) |
| At the merge | [Conflicts with main](#main-moved-under-it-and-the-merge-has-conflicts) · [G3 red on main](#g3-red-on-top-of-the-latest-main) · [Reviewer KO](#reviewer-ko-or-g3-red-after-one-fix) · [Main checkout not clean](#the-merge-needs-the-main-checkout-without-uncommitted-changes) |
| At G4 | [Not paused at G4](#not-paused-at-g4) · [Pick a variant](#pick-a-variant) |

## G0 pending: run the Setup

```text
error: G0 pending: run the Setup (the gdd-setup command).
```

**Cause.** No change starts before the Setup Gate (G0) is open. `bin/gdd status` shows `G0:
pending`.

**Fix.** Start your agent and run the `gdd-setup` command (`/gdd-setup` in Claude Code and
OpenCode, `$gdd-setup` in Codex). It writes the Brief, runs `bin/gdd selftest` and opens G0
with `bin/gdd gate G0 ok --by machines`.

## G0 cannot open

```text
error: G0 cannot open: the harness self-test is not 4/4 (bin/gdd selftest).
error: G0 cannot open: docs/brief.md (Project Brief) is missing.
error: G0 is opened by the machines (--by machines): Brief written and harness 4/4.
```

**Cause.** The machines open G0 when the Project Brief exists and the last self-test passed all
four checks, and nobody else can open it.

**Fix.** Write `docs/brief.md`, run `bin/gdd selftest` until it passes, then run `bin/gdd gate
G0 ok --by machines`. The self-test result is in `.gdd/selftest.json`.

## Self-test: G2 is red without the failing test

```text
KO a failing test turns G2 red (G2 is red without it: lint)
KO a failing test turns G2 red (gdd/debt.toml: invalid TOML: TOML parse error at line 17, column 1
```

**Cause.** The first check of the self-test needs G2 green before it plants its failing test.
It judges G2 as G2 does: a command held to its known debt in `gdd/debt.toml` does not make G2
red. The named command fails on main without known debt, or reports more than its count. In
the second form, `gdd/debt.toml` cannot be read, and the reason follows.

**Fix.** Make the named command pass on main, or, in a project that already has code, record
what fails today with `bin/gdd adopt` (see
[Bring gdd to an existing project](adopt-existing-project.md#4-record-the-known-debt-with-gdd-adopt)).
A command above its count needs its new problems fixed. Fix `gdd/debt.toml` when it cannot be
read. Then run `bin/gdd selftest` again.

## Red zones not approved yet

```text
error: Red zones not approved yet: the Orchestrator proposes them at the end of Discover. (bin/gdd gate zones ok --by human)
```

**Cause.** Until a human approves the red zones, only spikes can be built.

**Fix.** Read `gdd/zones.toml`, then approve them at the sitting of your change, or with
`bin/gdd gate zones ok --by human`. See [Map and approve red zones](red-zones.md).

## Red zones changed after their approval

```text
Red zones: pending · Red zones changed after their approval: a human must approve them again.
```

**Cause.** A red zone was added, removed or edited after its approval. Every change but a spike
is refused until it is approved again.

**Fix.** Read what changed in the history of `gdd/zones.toml`, then `bin/gdd gate zones ok --by
human`.

## New or changed scenarios without G1

```text
New or changed scenarios without G1: a human must approve them (the gdd-change command).
```

**Cause.** The change created or modified a `.feature` file outside `features/regression/`, and
nobody approved it. A G1 KO stops the launch the same way.

**Fix.** Hold the sitting (`bin/gdd sitting CHANGE`) or run `bin/gdd gate G1 ok CHANGE --by
human`. See [Write and approve scenarios](write-scenarios.md).

## Red change: a human must approve the plan

```text
error: Red change: a human must approve the plan (gate 'plan').
```

**Cause.** The change is red: it plans or touched a path of a red zone. A red change waits for a
human to approve its plan, in `.gdd/changes/CHANGE/plan.md`. A change can turn red on a
relaunch, when its Builder touched a red path: `bin/gdd classify CHANGE` counts what it touched.

**Fix.** Read the plan and approve it at the sitting, or with `bin/gdd gate plan ok CHANGE --by
human --note "..."`.

## Reviewer and Builder are the same model family

```text
error: Reviewer and Builder are the same model family: edit gdd/models.toml.
```

**Cause.** The Reviewer must be of another model family than the Builder, so it does not share
its blind spots. `gdd init` refuses the same way when every agent on the machine is of one
family:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

**Fix.** Give `[reviewer]` in `gdd/models.toml` an agent and a model of another family (see
`[families]` in the same file). If you have only one family, set the project up with `gdd init
--accept-same-family`: the change then says so in its Decision Inbox. See
[Choose agents and models](agents-and-models.md).

## Cannot find the program of an agent on PATH

```text
cannot find the claude program of Claude Code on PATH
error: no coding agent found on the PATH (claude, codex, opencode): install one or name it with --agents
```

**Cause.** The recipe checks that the program of every role's agent is on the `PATH` before
anything runs; `gdd init` looks for agents on the `PATH` when `--agents` is not given.

**Fix.** Install the agent and log in with its own command, or change the role in
`gdd/models.toml`. See [Install gdd](install.md).

## The working tree has uncommitted changes

```text
error: The working tree has uncommitted changes. Commit them before launching a change.
```

**Cause.** `gdd launch` moves the main checkout to main and runs the change in its own
worktree; it refuses uncommitted changes in tracked files outside the change's branch.

**Fix.** Commit or stash them, then launch again. On the change's own branch, the earlier work
is committed for you before the relaunch.

## Run gdd launch from the main checkout

```text
error: run gdd resume from the main checkout (/home/me/notes): the worktrees of the changes are made and removed from there
```

**Cause.** You ran `gdd launch` or `gdd resume` inside a change's worktree; the message names
the command.

**Fix.** `cd` to the main checkout and run it there.

## The cap of changes building is reached

```text
2 change(s) building, the cap is 2: cart-total, export-csv
login-rate-limit can launch once one of those building reaches G4 or is parked, or with a higher [parallel] max in gdd/rules.toml.
```

**Cause.** `[parallel] max` changes are already building. Changes parked or waiting at G4 do not
count.

**Fix.** Wait, or raise `[parallel] max` on main. See
[Run changes in parallel](parallel-changes.md).

## G1 refused: hidden examples use step phrasings

```text
error: G1 refused: hidden examples use step phrasings that no visible scenario has, so test mode can never write them. Reuse visible sentences with new data:
  - a notebook with one note "…" and no tags
  - the tags field of row # is empty
```

**Cause.** A hidden example uses a sentence no visible scenario has. Test mode never sees the
hidden examples, so it could never write that step.

**Fix.** Rewrite the hidden example with the sentences of the visible scenarios and new data, or
add the sentence to a visible scenario. Commit on the branch and approve again.

## gdd sitting needs a terminal

```text
error: gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok export-csv --by human (and gdd gate zones ok --by human, gdd gate plan ok export-csv --by human for a red change)
```

**Cause.** The sitting is a screen, and there is no terminal (a script, CI, an agent's chat).

**Fix.** Run it in a terminal, or record each part with the `gdd gate` commands the message
names.

## A Builder or Reviewer cannot open or close gates

```text
error: A Builder or Reviewer cannot open or close gates.
```

**Cause.** The command ran with `GDD_ROLE` set to an agent role. Agents cannot record approvals,
answer G4 or release.

**Fix.** Run it yourself, in a shell without `GDD_ROLE`.

## Parked: G2 still red after 3 retries and 1 replan

```text
Change CHANGE parked: G2 still red after 3 retries and 1 replan. Message in .gdd/changes/CHANGE/inbox.md
```

**Cause.** Three Builder rounds and one replan by the Orchestrator did not turn G2 green. The
numbers come from `[limits] builder_retries` and `replans` of `gdd/rules.toml`.

**Fix.** Read `.gdd/changes/CHANGE/g2.json` for what failed, and the entries below for Path
Guard and scope failures. See [Get a parked change going again](parked-change.md).

## Protected path for builder-build

```text
gdd/rules.toml: protected path for builder-build
```

**Cause.** Path Guard found a path the role must not touch since the build started: the harness
for everyone; the scenarios and the steps written in test mode for build mode; production code
for test mode. The paths are `[protected]` of `gdd/rules.toml`, as they were when the build
started.

A second cause catches people fixing a parked change by hand: a hand commit on a change's
branch that holds the agents' uncommitted work moves its build start, and Path Guard then blames
build mode for the steps test mode wrote.

**Fix.** Remove the agent's edit of the protected path. If you committed by hand, commit the
agents' work first, then yours, as
[Fix by hand in the worktree](parked-change.md#4-fix-by-hand-in-the-worktree) shows.

## The gate commands did not run: Path Guard must pass first

```text
{"name":"path-guard","tail":"gdd/rules.toml: protected path for builder-build\nThe gate commands did not run: Path Guard must pass first."}
```

**Cause.** Every gate runs Path Guard before its commands. When it fails, no command runs, so an
agent that rewrote `gdd/commands.toml` never gets its commands run.

**Fix.** The same as for [a protected path](#protected-path-for-builder-build).

## Scenario approved in G1 and modified afterwards

```text
{"ok":false,"failures":["features/export_csv.feature: scenario approved in G1 and modified afterwards"]}
```

**Cause.** A sealed scenario file changed. Path Guard stops every role for it.

**Fix.** Put the approved file back, or, if the scenario must change, commit the new version on
the branch and approve G1 again. See
[Change a scenario after its approval](write-scenarios.md#8-change-a-scenario-after-its-approval).

## Outside the package and red

```text
src/store/index.rs: outside the package and red (the change is yellow); remove it, or the Orchestrator widens the scope
```

**Cause.** In build mode, G2 also checks the scope: a path the change did not plan is accepted
when its color is not higher than the change's, and listed in the Decision Inbox; a higher one
fails, because the change would need gates it was not given.

**Fix.** Remove the file in the worktree and relaunch, or keep it: the relaunch classifies the
change with what it touched, so it turns red and waits for its plan to be approved. See
[Why changes park](parked-change.md#why-changes-park).

## Path Guard failed after test mode

```text
Change CHANGE parked: Path Guard failed after test mode: src/x.rs: protected path for builder-test. Message in .gdd/changes/CHANGE/inbox.md
```

**Cause.** The Builder in test mode wrote production code, which only build mode may write.

**Fix.** Remove those files from the worktree and relaunch. If it happens again, check that
`builder_test` in `[protected]` matches where your production code lives.

## Changed GDD state

```text
Change CHANGE parked: the Builder in build mode changed GDD state: .gdd/changes/CHANGE/change.json.
```

**Cause.** gdd checks around every agent call that the agent did not touch the state in `.gdd/`
of its change or the state every change shares. It puts the state back and parks the change.
The same check parks a change when the Reviewer edits a file (`the Reviewer changed ...:
Reviewers only read`).

**Fix.** Read the agent's output in `.gdd/changes/CHANGE/outputs/`; check that its instructions
are the ones gdd wrote (`bin/gdd agents check`), then relaunch.

## Did not answer within N seconds

```text
Change CHANGE parked: the Builder in build mode did not answer within 3600 seconds.
```

**Cause.** An agent ran past its time limit and gdd stopped it, with every process it started.
The limit is `[limits] agent_timeout_s` of `gdd/rules.toml`, 3600 seconds when the file does
not set it. gdd also parks a change whose agent exits with an error (`failed (exit N)`).

**Fix.** Run the agent by hand once to check its login and quota. Raise `agent_timeout_s` on main
if the work is just slow, then relaunch.

## The spike spent its budget

```text
parked: the spike spent 5.00 USD of its budget of 5.00 USD
```

**Cause.** The agent calls of a Spike, or of one of its variants, cost `[spike] max_usd`.

**Fix.** Read what it found so far, then raise the budget on main and relaunch, or answer with
what you know. See [Explore with a spike](spikes.md).

## Timed out: raise limits.command_seconds

```text
timed out (1800s): raise limits.command_seconds in gdd/rules.toml
```

**Cause.** A gate or release command ran longer than `[limits] command_seconds` (1800 seconds by
default) and gdd stopped it. The gate fails on that command.

**Fix.** On a slow machine, or with two changes in G3 at once, raise `command_seconds` in
`gdd/rules.toml` on main. It must be a whole number of seconds above 0, or every gate stops
with `limits.command_seconds must be a whole number of seconds above 0`.

## Mutation testing times out

```text
2 mutant(s) hung again: counted as caught
```

**Cause.** On the Rust sheet, a mutant can time out because it hangs or because the machine is
busy. When only timeouts are left, the command runs those mutants again, one at a time with at
least 120 seconds per test; one that hangs again counts as caught, one that now survives fails
the gate. Missed mutants get no second chance.

**Fix.** Nothing when the gate passes. When the whole mutation command times out, raise
`command_seconds`, or avoid running two changes in G3 at once (see
[Run changes in parallel](parallel-changes.md)). See
[Tune the gates](gates-and-commands.md) for the commands themselves.

## gdd/commands.toml has no commands for this gate

```text
gdd/commands.toml has no commands for this gate. The Setup fills them in.
```

**Cause.** Every command of the gate is empty. A gate with no commands fails (`G2 not
configured`): an empty gate would pass anything.

**Fix.** Fill in the commands, on main. See [Tune the gates](gates-and-commands.md).

## Zero scenarios counts as a failure

```text
Zero scenarios counts as a failure.
```

**Cause.** G3 counts the scenarios of `features/`, and `[acceptance] required = true` in
`gdd/rules.toml` makes zero a failure.

**Fix.** Write the first scenario (the Setup writes a smoke scenario). See
[Write and approve scenarios](write-scenarios.md).

## Main moved under it and the merge has conflicts

```text
main moved under it and the merge has conflicts in src/export.rs: merge main into change/export-csv in its worktree (/home/me/notes/../notes.worktrees/export-csv), settle the conflicts, commit, then gdd launch export-csv
```

**Cause.** Another change merged first, and git cannot merge the latest main into this one.

**Fix.** Follow the message: see
[Settle merge conflicts with main](parked-change.md#6-settle-merge-conflicts-with-main).

## G3 red on top of the latest main

```text
Change CHANGE parked: G3 red on top of the latest main. Message in .gdd/changes/CHANGE/inbox.md
```

**Cause.** Main merged cleanly into the change, but G3 fails with what main brought.

**Fix.** Read `g3.json`, fix it on the branch in the worktree, commit, relaunch.

## Reviewer KO or G3 red after one fix

```text
Change CHANGE parked: Reviewer KO or G3 red after one fix. Message in .gdd/changes/CHANGE/inbox.md
```

**Cause.** The Reviewer said KO, the Builder had its one fix, and the Reviewer said KO again (or
G3 broke during the fix).

**Fix.** Read the last `outputs/reviewer-<n>.md`. When only the steps can fix the finding, build
mode cannot (they are protected for it): fix them by hand. See
[Get a parked change going again](parked-change.md).

## The merge needs the main checkout without uncommitted changes

```text
Change CHANGE parked: the merge needs the main checkout (/home/me/notes) without uncommitted changes in tracked files: commit or stash them.
```

**Cause.** Someone edited a tracked file in the main checkout while the change ran, and the
merge happens there.

**Fix.** Commit or stash the edit, then `bin/gdd launch CHANGE`.

## Not paused at G4

```text
error: Change export-csv is not paused at G4.
```

**Cause.** `gdd resume` answers only a change waiting at G4. After a failed observation the
change was rolled back and no longer waits.

**Fix.** Check the state with `bin/gdd status`. A rolled-back change needs a hotfix. See
[Answer the Release Gate](release-gate.md).

## Pick a variant

```text
error: pick a variant: gdd resume notes-storage --g4 ok --variant 1|2
```

**Cause.** OK and polish on a Spike with variants need the number of the variant.

**Fix.** Add `--variant <n>` with a variant that passed its gate. See
[Explore with a spike](spikes.md#6-compare-the-variants-and-pick-one).

## Next steps

- [Get a parked change going again](parked-change.md): the full way back for a parked change.
- [Command reference](../reference/cli.md): every command and option.
- [Files gdd writes](../reference/files.md): where the reports, outputs and records live.
- [Keeping agents honest](../explanation/safety.md): why Path Guard and the state checks are this
  strict.
