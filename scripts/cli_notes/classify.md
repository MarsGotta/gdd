---
summary: Prints the color, lane and gates of a change, as JSON.
runs: your agents
---

## Who runs it

Your agent, as the Orchestrator in the [`gdd-replan`](../skills/agent-skills.md) skill, after
a replan.

## Examples

gdd also classifies a change when it creates it and when it prepares it.

```bash
gdd classify cart-total
```

You should see the color, the lane and the reason for each path:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{
  "change": "cart-total",
  "color": "yellow",
  "lane": "full",
  "reasons": [
    "src/shop/cart.py -> yellow (no zone: default color)",
    "tests/unit/test_cart.py -> yellow (no zone: default color)"
  ],
  "g1": false,
  "features": [],
...
```

## See also

- [How risk picks the gates](../../explanation/risk.md): where the color comes from.
