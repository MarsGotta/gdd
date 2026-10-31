# gdd-hotfix

The hotfix skill is for something broken in production. The Orchestrator rolls production
back first, then fixes the cause as a normal change of type Hotfix, with a scenario that
reproduces the failure. Run it when production is broken.

## Call it

Say what is broken. In Claude Code and OpenCode:

```text
/gdd-hotfix the CSV export writes an empty file since the last release
```

In Codex:

```text
$gdd-hotfix the CSV export writes an empty file since the last release
```

## What it asks you

- Nothing before the rollback: it rolls back and tells you in one line.
- Your OK for any shortcut, when rolling back is impossible.
- The usual OK on the fix's scenarios, at its sitting.

## What it does

1. **Mitigates first.** gdd rolls a change back by itself when production gets worse while it
   observes it; the change then ends `rolled-back` and its Decision Inbox asks for a hotfix.
   When that automatic rollback did not happen, the Orchestrator runs the `rollback` command of
   `[release]` in `gdd/commands.toml` and tells you in one line.
2. **Fixes it through its lane.** With production stable there is no hurry: it creates the
   fix as a change of type Hotfix, through [`gdd-change`](change.md), with a regression
   scenario in `features/regression/` that reproduces the failure. The fix meets the gates of
   its color, as any change does.
3. **Asks before a shortcut.** When rolling back is impossible, it explains in three lines what
   happens and what it proposes. A shortcut without a rollback needs your OK.
4. **Leaves a lesson**: a short note in `docs/learn/` with the cause and the new rule that
   would have prevented it.

## What you decide

- Whether to take a shortcut, when there is no rollback.
- The scenarios of the fix, at its sitting, and its G4, as for any change.

## What it leaves behind

| What | Where |
|---|---|
| Production on the previous version | after the rollback |
| The fix, as a change of type Hotfix | `.gdd/changes/<id>/`, branch `change/<id>` |
| The scenario that reproduces the failure | `features/regression/` |
| The cause and the new rule | `docs/learn/` |

## See also

- [Answer the Release Gate](../../how-to/release-gate.md#ok-promote-it): what a failed
  observation does.
- [`gdd release`](../cli/release.md): the release steps, observe included.
- [`gdd-learn`](learn.md): where the lesson goes next.
