# gdd-learn

The learn skill is Phase 5 of GDD. The Orchestrator reads the log and the closed and parked
changes, finds the errors that repeat, and proposes one rule or check for each. Learn has no
gate: it only hardens the ones you have.

## Call it

In Claude Code and OpenCode:

```text
/gdd-learn
```

In Codex:

```text
$gdd-learn
```

The input is optional. Use it to point at something you noticed:

```text
/gdd-learn changes keep parking on lint at G2
```

## What it asks you

Nothing until the end, when it brings you its proposals in a few lines, with its
recommendation.

## What it does

1. **Runs [`gdd learn`](../cli/learn.md) first**: it runs the Learn tools of
   `gdd/commands.toml` and lists the polish notes you gave at G4 and the classes of Reviewer
   finding that repeat.
2. **Reads the numbers and the history**: [`gdd report`](../cli/report.md),
   `gdd report --by phase,role`, and the closed and parked changes in `.gdd/changes/`.
3. **Looks for errors that repeat**: the same G2 failure in several changes, the same reason
   for a Reviewer KO, badly written scenarios, many retries with one model. Your polish notes
   at G4 and the Reviewer's improvements count too, when they repeat.
4. **Proposes one fix for each**: a rule, a lint, a hook or a test that catches it next time.
   A repeated polish note becomes a line of `docs/standards.md` or, when a machine can check
   it, a gate. It never loosens a gate.
5. **Looks for unused code and dependencies** with the stack's tool (knip, vulture or
   cargo-machete) and proposes removing them.
6. **Proposes a refactor** only when the log justifies it.
7. **Writes the proposal** in `docs/learn/YYYY-MM-DD.md` and brings it to you. A change to
   `gdd/` is red work, so it never makes it on its own.
8. **Counts the conversation** with [`gdd session --phase 5-learn`](../cli/session.md).

## What you decide

Which proposals to take. Nothing in `gdd/` changes without your OK.

## What it leaves behind

| What | Where |
|---|---|
| The proposals | `docs/learn/YYYY-MM-DD.md` |
| What each Learn tool said | `.gdd/learn/`, written by `gdd learn` |
| The conversation, counted | `.gdd/log.jsonl` |

## When to run it

From time to time: after several changes, after a change that parked, or when the numbers of
[`gdd-report`](report.md) look wrong.

## See also

- [Few stops for the human](../../explanation/human-stops.md#learn-turns-repeated-notes-into-rules):
  why repeated notes become rules.
- [From numbers to Learn](../../explanation/measuring.md#from-numbers-to-learn): which numbers
  point to what.
- [Gates](../gates.md#learn): what Learn may and may not change.
