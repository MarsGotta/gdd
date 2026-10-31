# gdd-status

The status skill tells you where the project stands in GDD and gives you one next step, with
the Orchestrator's recommendation. It changes nothing. Run it when you come back to a project,
or whenever you are unsure what comes next.

## Call it

In Claude Code and OpenCode:

```text
/gdd-status
```

In Codex:

```text
$gdd-status
```

You can add a question, and it answers from the same state:

```text
/gdd-status is anything waiting for me?
```

## What it asks you

Nothing. It answers in a few lines.

## What it does

1. Reads `AGENTS.md`, the project's instructions for agents, if it has not read it in this
   session: every project `gdd init` set up has one. Then runs [`gdd status`](../cli/status.md):
   G0, the red zones, the self-test, the scenarios, the changes and the log.
2. Picks the one next step, in this order:

   | When | It proposes |
   |---|---|
   | G0 is pending | [`gdd-setup`](setup.md): the project is in Phase 0 |
   | G0 is open and no Discover record covers the next work | [`gdd-discover`](discover.md) |
   | The red zones are pending or changed | approving them with the scenarios of the next change, in [`gdd-change`](change.md) |
   | A change waits at G4 | your answer: OK, KO or polish |
   | A change parked | what its Decision Inbox, `.gdd/changes/<id>/inbox.md`, says |
   | No change is running | the next change, with [`gdd-change`](change.md) |

3. Answers: where you are and one next step.

## What you decide

Whether to take the step it proposes.

## What it leaves behind

Nothing: `gdd status` only reads.

## What comes next

The skill it proposed. It is also the way back after a pause: start your agent again, run
`gdd-status`, and it finds a change waiting for your G4 or a parked one.

## See also

- [`gdd status`](../cli/status.md): the command, and every line it prints.
- [The Board](../screens.md#the-board): the same changes by phase, on a screen.
