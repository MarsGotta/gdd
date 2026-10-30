# Get a parked change going again

A change parks when it reaches a limit that only a person can lift: the gates stay red after
every retry, the Reviewer still says KO, or main moved under it with conflicts. This page shows
how to read why, fix it and launch the change again, or withdraw it.

## Before you start

- A change that `bin/gdd status` lists as `parked`. `gdd launch` exits with 1 and prints the
  reason when a change parks: `Change export-csv parked: <reason>. Message in
  .gdd/changes/export-csv/inbox.md`.
- Its worktree: a parked change keeps it, next to the project by default
  (`../notes.worktrees/export-csv` for the project `notes`), or in `[parallel] folder` of
  `gdd/rules.toml`. The agents' work is there.

The examples use the change `export-csv` of the project `notes`.

## The park and relaunch loop

A parked change waits for you; each relaunch runs the recipe again from the start, and can park
again for another reason.

```mermaid
stateDiagram-v2
    building: building
    parked: parked
    fixed: fixed by hand in the worktree
    building --> parked: a limit is reached
    parked --> fixed: read the Decision Inbox, fix, commit
    parked --> building: gdd launch (nothing to fix by hand)
    fixed --> building: gdd launch
    building --> [*]: merged, or paused at G4
    parked --> [*]: withdrawn, stays parked
    classDef human fill:#e6e0f8,stroke:#6a4fc0,color:#213
    class fixed human
```

## Why changes park

Every loop of the recipe has a limit (`[limits]` of `gdd/rules.toml`), and the reason names the
one reached:

| Reason | What happened | Usual way out |
|---|---|---|
| `G2 still red after 3 retries and 1 replan` | The Builder could not turn the Loop Gate (G2) green: lint, types, tests, Path Guard or the scope. The Orchestrator's replan did not help either. | Read `g2.json`. Fix what the Builder could not: the package, a scenario, a step, the harness. |
| `G3 still red after one fix` | The Merge Gate (G3) failed, and one more Builder round did not fix it. | Read `g3.json`: a scenario, a hidden example, security analysis or a surviving mutant. |
| `Reviewer KO or G3 red after one fix` | The Reviewer said KO twice, or G3 broke during the fix. | Read the last `outputs/reviewer-<n>.md`. A finding only the steps can fix needs a hand fix: build mode cannot touch them. |
| `main moved under it and the merge has conflicts in ...` | Another change merged first and git cannot merge main into this one. | [Settle the conflicts](#6-settle-merge-conflicts-with-main). |
| `G3 red on top of the latest main` | Main merged cleanly, but G3 fails with it. | Fix it on the branch, as for G3 red. |
| `Path Guard failed after test mode: ...` | The Builder in test mode wrote what only build mode may write, such as production code. | Remove those files from the worktree. |
| `the Builder in build mode changed GDD state: ...` | An agent wrote in `.gdd/`. gdd put the state back. | Read the agent's output; check its instructions. |
| `the Reviewer changed ...: Reviewers only read` | The Reviewer edited a file. | Remove its edits; check its agent sheet. |
| `the Builder in build mode did not answer within 3600 seconds` | The agent ran out of time and was stopped. | Check the agent's login and quota; `[limits] agent_timeout_s` sets the time. |
| `the spike spent ... USD of its budget of ... USD` | A Spike reached `[spike] max_usd`. | See [Explore with a spike](spikes.md). |
| `the merge needs the main checkout (...) without uncommitted changes in tracked files: commit or stash them.` | Someone edited a tracked file in the main checkout while the change ran. | Commit or stash it, then relaunch. |
| `production may have it, but gdd cannot go on with it (...)` | The change was caught in the middle of a promotion. | Look at production first: see [Answer the Release Gate](release-gate.md). |

A path outside the plan that has a higher color than the change also fails G2, under `scope`:

```text
src/store/index.rs: outside the package and red (the change is yellow); remove it, or the Orchestrator widens the scope
```

`gdd classify` counts what the change really touched, so after such a park it shows the change
red. To keep the path, relaunch: the change is now red and `gdd launch` stops at `Red change: a
human must approve the plan (gate 'plan').` until you approve its plan
(`bin/gdd gate plan ok export-csv --by human`). To drop the path, remove the file in the
worktree, then relaunch.

## 1. Read the Decision Inbox

```bash
bin/gdd inbox export-csv
```

You should see something like this (here, a change parked with `gdd park`):

```text
# Decision Inbox · export-csv · Export notes as CSV

Color yellow · lane full · state parked

## Needs your decision
- Tags with spaces: one field joined by spaces, or one column per tag? I recommend one field.

## Decided without you
- The header stays in English whatever the language of the notes.

## Parked
- Waiting for the bank's CSV format before going on
```

The inbox is one message per change, also saved in `.gdd/changes/export-csv/inbox.md`:

- **Needs your decision**: questions the agents left, each with a recommendation.
- **Decided without you**: what the agents decided and why, including paths outside the plan
  that the scope check accepted, and warnings that the change overlaps another one.
- **Parked**: every reason the change parked, the latest last.
- **How it works**, first, when the Reviewer wrote one: a short explainer of the code.

A merge conflict reads like this in a real inbox (paths rewritten):

```text
## Parked
- Reviewer KO or G3 red after one fix
- main moved under it and the merge has conflicts in crates/gdd-core/src/change.rs, crates/gdd-core/src/recipe/mod.rs: merge main into change/builder-session in its worktree (/home/me/gdd/../gdd.worktrees/builder-session), settle the conflicts, commit, then gdd launch builder-session
```

## 2. Read what the gates and the agents said

Everything the run left is in `.gdd/changes/export-csv/`, in the main checkout:

| File | What it holds |
|---|---|
| `g2.json`, `g3.json` | The last report of each gate: what passed, what was skipped, and the failure lines of each failed command. |
| `review.md` | The Reviewer's verdict and its findings. |
| `replan.md` | The Orchestrator's replan: the cause and the new approach. |
| `outputs/<role>-<n>.md` | What each agent call said, numbered in order (`builder-build-3.md`, `reviewer-2.md`). |
| `package.md` | The context package the Builder worked from. |

`bin/gdd report --change export-csv` shows what the run cost so far.

## 3. Answer what needs you

Settle each question in the inbox. When the answer changes the work, put it where the agents
read it: the package, a scenario (which needs G1 again, see
[Write and approve scenarios](write-scenarios.md#8-change-a-scenario-after-its-approval)), the
plan, or the zone map on main. Leave a note of what you decided:

```bash
bin/gdd note export-csv decided "Tags go in one field, joined by spaces."
```

You should see:

```text
noted
```

Notes are `needs`, `decided` or `parked`. On the next launch, the questions of this run are
marked as resolved: the inbox keeps them in `change.json` and shows only how many there were,
for example `_1 earlier request(s) were resolved before the last launch (kept in
change.json)._`

## 4. Fix by hand in the worktree

Some causes need a hand fix: a step only test mode could write, a scenario that contradicts the
change, a harness problem. Work in the change's worktree, never on main:

```bash
cd ../notes.worktrees/export-csv
git status
```

> [!WARNING]
> Commit the agents' uncommitted work on the branch before any hand commit. A parked change
> keeps their work uncommitted in the worktree, and the relaunch starts its build at the commit
> checked out. Commit only your own files, and the agents' work falls after that start: Path
> Guard then blames build mode for the steps test mode wrote, and the change parks again.

```bash
git add -A
git commit -m "export-csv: the agents' work before the hand fix"
# now your fix
git add tests/acceptance/export_csv.rs
git commit -m "export-csv: hand fix of the export step"
git log --oneline -3
```

Check with `git log` and `git status` that both commits are there and the tree is clean. Then
go back to the main checkout:

```bash
cd ../../notes
```

When you commit nothing by hand, the relaunch commits the agents' work itself, as
`gdd(export-csv): work of the earlier run, before relaunch`, once the gates that judged it
accept it.

## 5. Launch it again

From the main checkout, on main, with no uncommitted changes in tracked files:

```bash
bin/gdd launch export-csv
```

A relaunch checks the change again (G0, red zones, G1, the plan of a red change, the model
families) and runs G2 on the code of the branch, judged under Path Guard from where the branch
left main. When that G2 is already green, the Builder is skipped and the recipe goes on to G3
and the Reviewer (`resume_green` in the record, `G2 already green: Builder skipped` in the log).
When it is red, the Build runs again from its start, with a full set of retries and a replan.

`gdd launch` refuses to run from a change's worktree (`run gdd launch from the main checkout
(...)`) and with uncommitted changes outside the change's branch (`The working tree has
uncommitted changes. Commit them before launching a change.`).

## 6. Settle merge conflicts with main

Merges go one at a time. When main moved since the change passed G3, gdd merges the latest main
into the change's branch and runs G3 again on top of it. When git cannot merge, gdd gives the
merge up, leaves the branch with its work committed (`gdd(export-csv): change work`) and parks
the change. gdd does not settle conflicts for you. In the worktree:

```bash
cd ../notes.worktrees/export-csv
git merge main
# settle every file git names, keeping what both sides added
git add -A
git commit --no-edit
```

Run the tests of the files you settled before you relaunch, then launch from the main checkout:

```bash
cd ../../notes
bin/gdd launch export-csv
```

The relaunch runs G3 on the merge and the Reviewer reads it too. To avoid the conflict next time,
see [Run changes in parallel](parallel-changes.md).

## 7. Withdraw a change

To stop a change for good, park it with the reason, which goes to its Decision Inbox:

```bash
bin/gdd park export-csv "Waiting for the bank's CSV format before going on"
```

You should see:

```text
Change export-csv parked: Waiting for the bank's CSV format before going on. Message in .gdd/changes/export-csv/inbox.md
```

`gdd park` always exits with 1, so a script that runs it stops there. The change keeps its
record, its branch and its worktree, and `gdd launch` can take it up again later. While it stays
parked, a change that plans an overlapping path launches with a warning that names it (see
[Run changes in parallel](parallel-changes.md)).

## Next steps

- [Troubleshooting](troubleshooting.md): more messages, their causes and fixes.
- [Run changes in parallel](parallel-changes.md): the merge queue, and how to keep conflicts
  rare.
- [Files gdd writes](../reference/files.md): the record, the log and the Decision Inbox.
- [The life of a change](../explanation/life-of-a-change.md): every step a relaunch runs again.
