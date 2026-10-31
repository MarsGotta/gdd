---
summary: Runs the recipe of a change: Build, G2, G3, the Reviewer, merge and pre.
runs: your agents
---

## Who runs it

Your agent, as the Orchestrator, in the last step of the [`gdd-change`](../skills/change.md)
skill, once you approved the change. An agent that gdd started cannot run it: see [What an
agent may run](../roles.md#what-an-agent-may-run).

## Examples

It starts the Builder and the Reviewer, so each launch costs agent calls. It builds in a git
worktree of its own, on the branch `change/<id>`, and moves your checkout to the main branch.

```bash
gdd launch cart-total
```

It prints nothing while the agents work, and pauses at G4 when the change needs you. When the
change parks, it exits with 1. The [tutorial](../../getting-started/tutorial.md) shows a whole
launch with real output.

## See also

- [Run a change](../../how-to/run-a-change.md): from the idea to production.
- [Run changes in parallel](../../how-to/parallel-changes.md): `[parallel] max` and overlaps.
- [Life of a change](../../explanation/life-of-a-change.md): every step of the recipe.
