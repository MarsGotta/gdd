# gdd-change

The change skill takes one change from your sentence to the Release Gate (G4). The
Orchestrator creates the change, writes its scenarios and its plan when it needs one, asks for
your OK once, hands the change to gdd's recipe and comes back when you must answer again. Run it
every time you want something built, fixed or reshaped.

## Before you start

- G0 is open: [`gdd-setup`](setup.md) is done.
- Your agent runs in the project folder, on `main`.

## Call it

Say what you want and why, in your own words. In Claude Code and OpenCode:

```text
/gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

In Codex:

```text
$gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

When you already have an id in mind, start with it: `/gdd-change export-csv export the notes
as CSV`. Ids are lowercase letters, digits and hyphens. Say what and why, not how: the code is
the agents' job.

## What it asks you

Everything it needs from you before the build comes in **one message**, the sitting:

- **The scenarios**, as plain text, when the change creates or changes visible behavior. You
  do not see the hidden examples, the extra cases kept from the Builder: they are there to
  catch code that fits only the visible ones.
- **The red zones**, when [`gdd status`](../cli/status.md) says they are pending or changed,
  or when this change creates a new sensitive area: a short list, each zone with its reason.
- **The plan**, when the change is red: what, how, the risks and how to roll back, in words.

You answer OK, or say what to change. Later, it comes back once more when the change waits at
G4, and it may leave questions for you in the change's Decision Inbox, each with its
recommendation.

## What it does

1. **Checks the project** with [`gdd status`](../cli/status.md): G0 must be open.
2. **Creates the change** with [`gdd new-change`](../cli/new-change.md): the id, a title, the
   type (Feature, Hotfix, Refactor, Migration, Spike or No-code) and the paths it expects to
   touch. gdd answers with the classification: the color, the lane and the gates. For a Spike
   with 2 to 4 options it asks for one variant per option, and for a Spike with nothing to
   tell apart, for several variants of the same brief.
3. **Writes the plan** of a red change in `.gdd/changes/<id>/plan.md`, never code.
4. **Writes the scenarios** on the change's branch `change/<id>`: `.feature` files in
   `features/`, in the words of the domain, few scenarios with concrete examples, plus one or
   two hidden examples in `features/hidden/`. Then it switches back to `main`.
5. **Holds the sitting** with you, as above. With your OK it records each part:
   - the scenarios: a commit on the branch, then
     [`gdd gate G1 ok <id> --by human`](../cli/gate.md), which seals them;
   - the red zones: a commit of `gdd/zones.toml` on `main`, then `gdd gate zones ok`;
   - the plan: `gdd gate plan ok <id>`.
6. **Writes the context package** in `.gdd/changes/<id>/package.md`, the only context the
   Builder gets: the goal in three lines, the color, the scenarios it covers, the paths it may
   touch, what to reuse, the rules of its zone and the G2 command.
7. **Leaves notes** in the Decision Inbox with [`gdd note`](../cli/note.md): `decided` for what
   it chose without you, `needs` for what needs your decision.
8. **Commits what is pending on `main` and launches** the change with
   [`gdd launch <id>`](../cli/launch.md). The recipe builds it (G2), integrates it (G3), has
   the Reviewer read it, merges it and deploys it to pre. That takes minutes.
9. **Brings G4 to you**, when the change pauses there: it shows you the Decision Inbox
   (`.gdd/changes/<id>/inbox.md`) and the demo. Your answer becomes
   [`gdd resume <id> --g4 ok`](../cli/resume.md), `--g4 ko`, or `--g4 polish --note` with your
   words, which runs one more Builder round and comes back to G4.
10. **Proposes the next step in one line** when the change parked, after reading its inbox.
11. **Counts the work** with [`gdd session --phase 1-discover --change <id>`](../cli/session.md),
    since this conversation (classification, scenarios, sitting) is the change's Discover phase,
    and shows what the change cost with [`gdd report --change <id>`](../cli/report.md).

## What you decide

| When | You decide | It becomes |
|---|---|---|
| The sitting | the scenarios: OK, or what to change | G1, the Scenario Gate |
| The sitting | the red zones, when they wait | the red zones' approval |
| The sitting | the plan of a red change | the plan's approval |
| On the way | the questions it leaves under *Needs your decision* | its next step |
| G4 | OK, KO or polish | promotion, rejection, or one more round |

Prefer a screen for the sitting or for G4? Run [`gdd sitting <id>`](../cli/sitting.md) in a
terminal: it records the same decisions. [Screens](../screens.md#the-sitting) shows it.

## What it leaves behind

| What | Where |
|---|---|
| The change's record: type, paths, color, gates, inbox | `.gdd/changes/<id>/change.json` |
| The branch, with the scenarios and their commit | `change/<id>` |
| The plan of a red change | `.gdd/changes/<id>/plan.md` |
| The Builder's context package | `.gdd/changes/<id>/package.md` |
| The Decision Inbox | `.gdd/changes/<id>/inbox.md` |
| Every gate and agent call | `.gdd/log.jsonl` |
| The approved red zones, when they waited | `gdd/zones.toml` on `main`, `.gdd/zones.json` |

After the launch, the change has its own worktree next to the project
(`../PROJECT.worktrees/<id>`) while it runs. Once merged, `main` has a merge commit that
carries the change's record. When it closes after your OK, the worktree goes and the branch
stays; with `[versions] mode = "auto"` it may also get a version tag.

## What comes next

- The change waits at G4 and the chat has ended? Run [`gdd-status`](status.md): it finds the
  change waiting for you.
- The change parked? See [Get a parked change going again](../../how-to/parked-change.md).
- The next change: run `gdd-change` again. Every few changes, [`gdd-learn`](learn.md).

## See also

- [Describe a change to your agent](../../how-to/run-a-change.md): this skill in use, with what
  you see at each step.
- [Approve the scenarios](../../how-to/write-scenarios.md): what to look for at the sitting.
- [Answer the Release Gate](../../how-to/release-gate.md): what to check before OK.
- [The life of a change](../../explanation/life-of-a-change.md): what the recipe does after
  the launch.
