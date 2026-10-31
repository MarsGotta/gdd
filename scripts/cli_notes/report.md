---
summary: Tokens, cost and time of the agent calls; with --time or --dora, the flow.
runs: you, your agents
---

## Who runs it

You, or your agent in the [`gdd-report`](../skills/report.md) and
[`gdd-learn`](../skills/learn.md) skills.

## Examples

```bash
gdd report --time
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
cart-total · Show the cart total · in progress
  Flow time 26s · active 4s (14%) · waiting 15s · between steps 7s
  Gates: security 2s, secrets 1s, acceptance 0s, acceptance_full 0s, unit 0s, architecture 0s, dependencies 0s, lint 0s, types 0s, lockfile 0s, path-guard 0s
  Waiting: to start 15s

Project: 1 change(s) · flow time 26s · active 4s (14%) · waiting 15s · between steps 7s
Biggest waits: to start 15s
```

Before any agent call:

```text
No agent calls in the log yet.
```

## See also

- [Measure the work](../../how-to/measure.md): every report and what to read in it.
- [Measuring](../../explanation/measuring.md): why these numbers.
