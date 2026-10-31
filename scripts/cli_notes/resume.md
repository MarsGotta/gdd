---
summary: Answers the G4 of a paused change: ok, ko or polish.
runs: you, your agents
---

## Who runs it

Your agent, as the Orchestrator, with your answer, in the [`gdd-change`](../skills/change.md)
skill; or you, on the G4 page of [`gdd sitting`](sitting.md), which runs the same answer. An
agent that gdd started cannot run it: see [What an agent may
run](../roles.md#what-an-agent-may-run).

## Examples

```bash
gdd resume cart-total --g4 ok
gdd resume cart-total --g4 polish --note "Show the total with its currency"
```

`ok` takes the change to production, `ko` rejects it, and `polish` runs one more Builder round
with your note, then pauses at G4 again. The [tutorial](../../getting-started/tutorial.md)
shows a real G4 answer.

## See also

- [Answer the Release Gate](../../how-to/release-gate.md): what to look for at G4.
