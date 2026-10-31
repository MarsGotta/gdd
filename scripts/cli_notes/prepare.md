---
summary: Checks that a change may enter Build, and starts it.
runs: your agents
---

## Who runs it

[`gdd launch`](launch.md), as the first step of its recipe; by hand, only to drive a change
without agents.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It runs on the branch `change/<id>` and checks G0, the red zones, G1, the plan, the model
families and the branch.

On the branch `change/cart-total`:

```bash
gdd prepare cart-total
```

You should see its decision, with the model of each role:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true,"change":"cart-total","color":"yellow","lane":"full",...}
```

## See also

- [Life of a change](../../explanation/life-of-a-change.md): every step of the recipe.
