---
summary: Merges a change whose G3 is open into main, then deploys it to pre.
runs: your agents
---

## Who runs it

[`gdd launch`](launch.md), once G3 and the Reviewer pass; by hand, only to drive a change
without agents.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

```bash
gdd merge cart-total
```

It prints the deploy as JSON. The [tutorial](../../getting-started/tutorial.md) shows a real
merge.

## See also

- [Life of a change](../../explanation/life-of-a-change.md): every step of the recipe.
