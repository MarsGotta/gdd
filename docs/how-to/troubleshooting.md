# Troubleshooting

Find the message gdd printed, or the way a change stopped, and do what its entry says. Each
entry shows what you see, why it happens in one line, and what to do.

Messages are quoted as gdd prints them. `CHANGE` stands for the id of your change, and the
numbers, paths and ids in the messages are examples. If a change parked, start with
[Get a parked change going again](parked-change.md).

## Before you start

- Status: run [`gdd status`](../reference/cli/status.md) first. It says where G0, the red
  zones, the self-test and every change stand.
- Inbox: for one change, [`gdd inbox CHANGE`](../reference/cli/inbox.md) shows its Decision
  Inbox. `.gdd/changes/CHANGE/` holds the gate reports (`g2.json`, `g3.json`) and what each
  agent said (`outputs/`).

## Find your message

| When | Entries |
|---|---|
| Setting up | [The first install failed](#the-first-install-failed) |
| Before the build starts | [G0 pending](#g0-pending-run-the-setup) · [G0 cannot open](#g0-cannot-open) · [Self-test: G2 red](#self-test-g2-is-red-without-the-failing-test) · [Red zones not approved](#red-zones-not-approved-yet) · [Red zones changed](#red-zones-changed-after-their-approval) · [Scenarios without G1](#new-or-changed-scenarios-without-g1) · [Red plan](#red-change-a-human-must-approve-the-plan) · [Same model family](#reviewer-and-builder-are-the-same-model-family) · [Agent not found](#cannot-find-the-program-of-an-agent-on-path) · [Uncommitted changes](#the-working-tree-has-uncommitted-changes) · [Not the main checkout](#run-gdd-launch-from-the-main-checkout) · [The cap](#the-cap-of-changes-building-is-reached) |
| At a sitting | [G1 refused](#g1-refused-hidden-examples-use-step-phrasings) · [No terminal](#gdd-sitting-needs-a-terminal) · [Builder cannot record](#a-builder-or-reviewer-cannot-open-or-close-gates) · [Agent cannot run a command](#builders-and-reviewers-cannot-run-gdd-command) |
| During the build | [G2 still red](#parked-g2-still-red-after-3-retries-and-1-replan) · [Protected path](#protected-path-for-builder-build) · [Commands did not run](#the-gate-commands-did-not-run-path-guard-must-pass-first) · [Approved scenario modified](#scenario-approved-in-g1-and-modified-afterwards) · [Outside the package](#outside-the-package-and-red) · [Test mode wrote code](#path-guard-failed-after-test-mode) · [Agent changed GDD state](#changed-gdd-state) · [Agent did not answer](#did-not-answer-within-n-seconds) · [Spike budget](#the-spike-spent-its-budget) |
| In G2 and G3 | [Command timed out](#timed-out-raise-limitscommand_seconds) · [Mutation timeouts](#mutation-testing-times-out) · [Not configured](#gddcommandstoml-has-no-commands-for-this-gate) · [Zero scenarios](#zero-scenarios-counts-as-a-failure) |
| At the merge | [Conflicts with main](#main-moved-under-it-and-the-merge-has-conflicts) · [G3 red on main](#g3-red-on-top-of-the-latest-main) · [Reviewer KO](#reviewer-ko-or-g3-red-after-one-fix) · [Main checkout not clean](#the-merge-needs-the-main-checkout-without-uncommitted-changes) |
| At G4 | [Not paused at G4](#not-paused-at-g4) · [Pick a variant](#pick-a-variant) |

## The first install failed

<!-- Real output of the gdd init form, gdd 0.1.0, with an install made to fail on purpose; the
     line the failing tool printed is left out. -->

```text
python: uv lock && uv sync --locked
...
error: python: uv lock && uv sync --locked failed (exit 2)
The harness is set up and committed. Fix the install, then run gdd bootstrap.
```

**Why.** The Setup form wrote the harness and committed it as `gdd init 0.1.0`, then the first
install of a stack failed: no network, a missing tool such as `uv`, `npm` or `cargo`, or a
dependency that does not resolve.

**What to do.** Read the line above the error: it is what the tool said. Fix that, then run
[`gdd bootstrap`](../reference/cli/bootstrap.md), which runs the install again and commits the
lock files. Do not run `gdd init` again, because the project is already set up. Then start your
agent and run the `gdd-setup` command.

## G0 pending: run the Setup

```text
error: G0 pending: run the Setup (the gdd-setup command).
```

**Why.** No change starts before the Setup Gate (G0) is open, and `gdd status` shows
`G0: pending`.

**What to do.** Start your agent and run the `gdd-setup` command (`/gdd-setup` in Claude Code
and OpenCode, `$gdd-setup` in Codex). It writes the Brief, runs
[`gdd selftest`](../reference/cli/selftest.md) and opens G0 with
[`gdd gate G0 ok --by machines`](../reference/cli/gate.md).

## G0 cannot open

```text
error: G0 cannot open: the harness self-test is not 4/4 (gdd selftest).
error: G0 cannot open: docs/brief.md (Project Brief) is missing.
error: G0 is opened by the machines (--by machines): Brief written and harness 4/4.
```

**Why.** Only the machines open G0, and only once the Project Brief exists and the last
self-test passed all four checks.

**What to do.** Write `docs/brief.md`, run `gdd selftest` until it passes, then run
`gdd gate G0 ok --by machines`. The self-test result is in `.gdd/selftest.json`.

## Self-test: G2 is red without the failing test

```text
KO a failing test turns G2 red (G2 is red without it: lint)
KO a failing test turns G2 red (gdd/debt.toml: invalid TOML: TOML parse error at line 17, column 1
```

**Why.** The self-test needs G2 green before it plants its failing test, and the named command
fails on main (beyond its known debt, if it has any), or `gdd/debt.toml` cannot be read.

**What to do.**

- Command named: make it pass on main. In a project that already has code, record what fails
  today with [`gdd adopt`](../reference/cli/adopt.md) (see [Bring gdd to an existing
  project](adopt-existing-project.md#3-known-debt-what-fails-today)). If the command reports
  more than its count, fix its new problems.
- `gdd/debt.toml` named: fix the file at the line the message gives.

Then run `gdd selftest` again.

## Red zones not approved yet

```text
error: Red zones not approved yet: the Orchestrator proposes them at the end of Discover. (gdd gate zones ok --by human)
```

**Why.** Until a person approves the red zones, only Spikes can be built.

**What to do.** Read `gdd/zones.toml`, then approve the zones at the sitting of your change, or
with `gdd gate zones ok --by human`. See [Map and approve red zones](red-zones.md).

## Red zones changed after their approval

```text
Red zones: pending · Red zones changed after their approval: a human must approve them again.
```

**Why.** Someone added, removed or edited a red zone after its approval, so gdd refuses every
change but a Spike until a person approves the zones again.

**What to do.** Read what changed in the history of `gdd/zones.toml`, then run
`gdd gate zones ok --by human`.

## New or changed scenarios without G1

```text
New or changed scenarios without G1: a human must approve them (the gdd-change command).
```

**Why.** The change created or modified a `.feature` file outside `features/regression/` and
nobody approved it, or G1 got a KO.

**What to do.** Hold the sitting ([`gdd sitting CHANGE`](../reference/cli/sitting.md)), or run
`gdd gate G1 ok CHANGE --by human`. See [Approve the scenarios](write-scenarios.md).

## Red change: a human must approve the plan

```text
error: Red change: a human must approve the plan (gate 'plan').
```

**Why.** The change plans or touched a path of a red zone, so it waits for a person to approve
its plan in `.gdd/changes/CHANGE/plan.md`.

**What to do.** Read the plan and approve it at the sitting, or with
`gdd gate plan ok CHANGE --by human --note "..."`. If the change turned red on a relaunch, its
Builder touched a red path: [`gdd classify CHANGE`](../reference/cli/classify.md) shows what it
counts.

## Reviewer and Builder are the same model family

```text
error: Reviewer and Builder are the same model family: edit gdd/models.toml.
```

**Why.** The Reviewer must come from another model family than the Builder, so it does not
share the Builder's blind spots.

[`gdd init`](../reference/cli/init.md) refuses the same way when every agent on the machine is
of one family:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

**What to do.** Give `[reviewer]` in `gdd/models.toml` an agent and a model of another family
(see `[families]` in the same file). If you have only one family, set the project up with
`gdd init --accept-same-family`, and each change then says so in its Decision Inbox. See
[Choose agents and models](agents-and-models.md).

## Cannot find the program of an agent on PATH

```text
cannot find the claude program of Claude Code on PATH
error: no coding agent found on the PATH (claude, codex, opencode): install one or name it with --agents
```

**Why.** The program of a role's agent is not on the `PATH`, which the recipe checks before
anything runs and `gdd init` searches when `--agents` is not given.

**What to do.** Install the agent and log in with its own command, or change the role in
`gdd/models.toml`. See [Install gdd](install.md).

## The working tree has uncommitted changes

```text
error: The working tree has uncommitted changes. Commit them before launching a change.
```

**Why.** [`gdd launch`](../reference/cli/launch.md) moves the main checkout to main, so it
refuses uncommitted changes in tracked files outside the change's branch.

**What to do.** Commit or stash them, then launch again. On the change's own branch, gdd commits
the earlier work for you before the relaunch.

## Run gdd launch from the main checkout

```text
error: run gdd resume from the main checkout (/home/me/notes): the worktrees of the changes are made and removed from there
```

**Why.** You ran `gdd launch` or [`gdd resume`](../reference/cli/resume.md) inside a change's
worktree; the message names the command.

**What to do.** Go to the main checkout (`cd` to the folder the message names) and run the
command there.

## The cap of changes building is reached

```text
2 change(s) building, the cap is 2: cart-total, export-csv
login-rate-limit can launch once one of those building reaches G4 or is parked, or with a higher [parallel] max in gdd/rules.toml.
```

**Why.** `[parallel] max` changes are already building; changes parked or waiting at G4 do not
count.

**What to do.** Wait, or raise `[parallel] max` on main. See
[Run changes in parallel](parallel-changes.md).

## G1 refused: hidden examples use step phrasings

```text
error: G1 refused: hidden examples use step phrasings that no visible scenario has, so test mode can never write them. Reuse visible sentences with new data:
  - a notebook with one note "…" and no tags
  - the tags field of row # is empty
```

**Why.** A hidden example uses a sentence no visible scenario has, and test mode, which never
sees hidden examples, could never write that step.

**What to do.** Rewrite the hidden example with the sentences of the visible scenarios and new
data, or add the sentence to a visible scenario. Commit on the branch and approve again.

## gdd sitting needs a terminal

```text
error: gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok export-csv --by human (and gdd gate zones ok --by human, gdd gate plan ok export-csv --by human for a red change)
```

**Why.** The sitting is a screen, and there is no terminal to show it in (a script, CI, an
agent's chat).

**What to do.** Run it in a terminal, or record each part with the `gdd gate` commands the
message names.

## A Builder or Reviewer cannot open or close gates

```text
error: A Builder or Reviewer cannot open or close gates.
```

**Why.** The command ran with `GDD_ROLE` set to an agent role, and agents cannot record
approvals, answer G4 or release.

**What to do.** Run it yourself, in a shell without `GDD_ROLE`.

## Builders and Reviewers cannot run gdd COMMAND

```text
error: Builders and Reviewers cannot run gdd selftest: an agent gdd started may only run gdd g2, gdd guard, gdd note, gdd status, gdd classify, gdd inbox, gdd report and gdd scope.
```

**Why.** gdd sets `GDD_ROLE` for every agent it starts, and such an agent may run only the
commands the message lists ([Roles](../reference/roles.md#what-an-agent-may-run) explains why).

**What to do.**

- Agent: if an agent hit it, there is nothing to fix. The agent tried something outside its
  work, and nothing happened.
- Your own shell: if you see it there, `GDD_ROLE` is set in your shell by mistake. Run
  `unset GDD_ROLE` and try again.

## Parked: G2 still red after 3 retries and 1 replan

```text
Change CHANGE parked: G2 still red after 3 retries and 1 replan. Message in .gdd/changes/CHANGE/inbox.md
```

**Why.** Three Builder rounds and one replan by the Orchestrator did not turn G2 green; the
numbers come from `[limits] builder_retries` and `replans` of `gdd/rules.toml`.

**What to do.** Read `.gdd/changes/CHANGE/g2.json` for what failed. For Path Guard and scope
failures, see the entries below. See [Get a parked change going again](parked-change.md).

## Protected path for builder-build

```text
gdd/rules.toml: protected path for builder-build
```

**Why.** Path Guard found a path the role must not touch: the harness for everyone, the
scenarios and steps for build mode, production code for test mode (`[protected]` of
`gdd/rules.toml`, as it was when the build started).

**What to do.**

1. Remove the agent's edit of the protected path.
2. If you committed by hand on the change's branch while it held the agents' uncommitted work,
   Path Guard now blames build mode for the steps test mode wrote. Commit the agents' work
   first, then yours, as [Fix by hand in the
   worktree](parked-change.md#4-fix-by-hand-in-the-worktree) shows.

## The gate commands did not run: Path Guard must pass first

```text
{"name":"path-guard","tail":"gdd/rules.toml: protected path for builder-build\nThe gate commands did not run: Path Guard must pass first."}
```

**Why.** Every gate runs Path Guard before its commands, and when it fails no command runs, so
an agent that rewrote `gdd/commands.toml` never gets its commands run.

**What to do.** Do the same as for [a protected path](#protected-path-for-builder-build).

## Scenario approved in G1 and modified afterwards

```text
{"ok":false,"failures":["features/export_csv.feature: scenario approved in G1 and modified afterwards"]}
```

**Why.** A sealed scenario file changed, and Path Guard stops every role for it.

**What to do.** Put the approved file back. If the scenario must change, commit the new version
on the branch and approve G1 again. See [Change a scenario after its
approval](write-scenarios.md#5-change-a-scenario-after-its-approval).

## Outside the package and red

```text
src/store/index.rs: outside the package and red (the change is yellow); remove it, or the Orchestrator widens the scope
```

**Why.** In build mode, G2 fails a path the change did not plan when its color is higher than
the change's, because the change would need gates it was not given.

**What to do.** Choose one:

- Remove: delete the file in the worktree and relaunch.
- Keep: relaunch as it is. The relaunch classifies the change with what it touched, so it turns
  red and waits for its plan to be approved.

See [Why changes park](parked-change.md#why-changes-park).

## Path Guard failed after test mode

```text
Change CHANGE parked: Path Guard failed after test mode: src/x.rs: protected path for builder-test. Message in .gdd/changes/CHANGE/inbox.md
```

**Why.** The Builder in test mode wrote production code, which only build mode may write.

**What to do.** Remove those files from the worktree and relaunch. If it happens again, check
that `builder_test` in `[protected]` matches where your production code lives.

## Changed GDD state

```text
Change CHANGE parked: the Builder in build mode changed GDD state: .gdd/changes/CHANGE/change.json.
```

**Why.** An agent touched the state gdd keeps in `.gdd/`, so gdd put the state back and parked
the change.

**What to do.** Read the agent's output in `.gdd/changes/CHANGE/outputs/`. Check that its
instructions are the ones gdd wrote ([`gdd agents check`](../reference/cli/agents.md)), then
relaunch. A Reviewer that edits a file parks the change the same way
(`the Reviewer changed ...: Reviewers only read`).

## Did not answer within N seconds

```text
Change CHANGE parked: the Builder in build mode did not answer within 3600 seconds.
```

**Why.** An agent ran past `[limits] agent_timeout_s` of `gdd/rules.toml` (3600 seconds when
the file does not set it), and gdd stopped it with every process it started.

**What to do.** Run the agent by hand once to check its login and quota. If the work is only
slow, raise `agent_timeout_s` on main, then relaunch. A change whose agent exits with an error
parks the same way, with `failed (exit N)`.

## The spike spent its budget

```text
parked: the spike spent 5.00 USD of its budget of 5.00 USD
```

**Why.** The agent calls of a Spike, or of one of its variants, cost `[spike] max_usd`.

**What to do.** Read what it found so far. Then either raise the budget on main and relaunch, or
answer with what you know. See [Explore with a spike](spikes.md).

## Timed out: raise limits.command_seconds

```text
timed out (1800s): raise limits.command_seconds in gdd/rules.toml
```

**Why.** A gate or release command ran longer than `[limits] command_seconds` (1800 seconds by
default), so gdd stopped it and the gate fails on that command.

**What to do.** On a slow machine, or with two changes in G3 at once, raise `command_seconds` in
`gdd/rules.toml` on main. Use a whole number of seconds above 0, or every gate stops with
`limits.command_seconds must be a whole number of seconds above 0`.

## Mutation testing times out

```text
2 mutant(s) hung again: counted as caught
```

**Why.** On the Rust sheet, mutants that timed out run again one at a time, and one that hangs
again counts as caught, while one that now survives fails the gate.

**What to do.** If the gate passes, nothing. If the whole mutation command times out, raise
`command_seconds`, or avoid running two changes in G3 at once (see
[Run changes in parallel](parallel-changes.md)). For the commands themselves, see
[Tune the gates](gates-and-commands.md).

## gdd/commands.toml has no commands for this gate

```text
gdd/commands.toml has no commands for this gate. The Setup fills them in.
```

**Why.** Every command of the gate is empty, and a gate with no commands fails
(`G2 not configured`) because it would pass anything.

**What to do.** Fill in the commands, on main. See [Tune the gates](gates-and-commands.md).

## Zero scenarios counts as a failure

```text
Zero scenarios counts as a failure.
```

**Why.** G3 counts the scenarios of `features/`, and `[acceptance] required = true` in
`gdd/rules.toml` makes zero a failure.

**What to do.** Write the first scenario (the Setup writes a smoke scenario). See
[Approve the scenarios](write-scenarios.md).

## Main moved under it and the merge has conflicts

```text
main moved under it and the merge has conflicts in src/export.rs: merge main into change/export-csv in its worktree (/home/me/notes/../notes.worktrees/export-csv), settle the conflicts, commit, then gdd launch export-csv
```

**Why.** Another change merged first, and git cannot merge the latest main into this one.

**What to do.** Follow the message, step by step: see
[Settle merge conflicts with main](parked-change.md#6-settle-merge-conflicts-with-main).

## G3 red on top of the latest main

```text
Change CHANGE parked: G3 red on top of the latest main. Message in .gdd/changes/CHANGE/inbox.md
```

**Why.** Main merged cleanly into the change, but G3 fails with what main brought.

**What to do.** Read `g3.json`, fix the failure on the branch in the worktree, commit and
relaunch.

## Reviewer KO or G3 red after one fix

```text
Change CHANGE parked: Reviewer KO or G3 red after one fix. Message in .gdd/changes/CHANGE/inbox.md
```

**Why.** The Reviewer said KO, the Builder had its one fix, and the Reviewer said KO again, or
G3 broke during the fix.

**What to do.** Read the last `outputs/reviewer-<n>.md`. If only the steps can fix the finding,
fix them by hand: build mode cannot, because they are protected for it. See [Get a parked
change going again](parked-change.md).

## The merge needs the main checkout without uncommitted changes

```text
Change CHANGE parked: the merge needs the main checkout (/home/me/notes) without uncommitted changes in tracked files: commit or stash them.
```

**Why.** Someone edited a tracked file in the main checkout while the change ran, and the merge
happens there.

**What to do.** Commit or stash the edit, then run `gdd launch CHANGE`.

## Not paused at G4

```text
error: Change export-csv is not paused at G4.
```

**Why.** `gdd resume` answers only a change waiting at G4, and a change rolled back after a
failed observation no longer waits.

**What to do.** Check the state with `gdd status`. If the change was rolled back, it needs a
Hotfix. See [Review a change at G4](release-gate.md).

## Pick a variant

```text
error: pick a variant: gdd resume notes-storage --g4 ok --variant 1|2
```

**Why.** An OK or a polish on a Spike with variants needs the number of the variant.

**What to do.** Add `--variant <n>` with a variant that passed its gate. See
[Explore with a spike](spikes.md#6-compare-the-variants-and-pick-one).

## Next steps

- [Get a parked change going again](parked-change.md): the full way back for a parked change.
- [Command reference](../reference/cli/index.md): every command and option.
- [Files gdd writes](../reference/files.md): where the reports, outputs and records live.
- [Keeping agents honest](../explanation/safety.md): why Path Guard and the state checks are
  this strict.
