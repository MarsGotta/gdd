---
summary: Creates a change: its record and its branch change/<id>.
runs: your agents
---

## Who runs it

Your agent, as the Orchestrator, in the [`gdd-change`](../skills/change.md) skill, and in the
[`gdd-discover`](../skills/discover.md) skill for Spikes.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

```bash
gdd new-change cart-total --title "Show the cart total" \
  --paths src/shop/cart.py tests/unit/test_cart.py
```

You should see how gdd classified the change:

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
...
```

## See also

- [Run a change](../../how-to/run-a-change.md): from the idea to production.
- [Spikes](../../how-to/spikes.md): `--type Spike`, `--option` and `--variants`.
