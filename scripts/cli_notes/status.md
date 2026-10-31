---
summary: Where the project stands: G0, red zones, self-test, scenarios, changes and log.
runs: you, your agents
---

## Who runs it

You, at any time, and your agent: the Orchestrator runs it first in the
[`gdd-change`](../skills/change.md) skill, and the [`gdd-status`](../skills/status.md) skill
sums it up for you.

## Examples

```bash
gdd status
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Project: shop · language en · platform cli
Brief: docs/brief.md
G0: ok · G0 ok
Red zones: approved · red zones approved
Harness self-test: harness 4/4
Scenarios in features/: 0
Security exceptions: none
Known debt: none
Changes:
  - cart-total · Show the cart total · building · no gates yet
  - export-csv · Export orders as CSV · parked · no gates yet
Log: 22 lines in .gdd/log.jsonl (gdd report)
```
