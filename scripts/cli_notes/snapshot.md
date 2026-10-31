---
summary: Records what a change touched, so Path Guard judges only what comes after.
runs: your agents
---

## Who runs it

[`gdd launch`](launch.md), after the Builder's test mode; by hand, only to drive a change
without agents.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

The change must be prepared first.

```bash
gdd snapshot cart-total
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true}
```

Before [`gdd prepare`](prepare.md), it refuses:

```text
error: change cart-total has not been prepared: its record has no build_start
```
