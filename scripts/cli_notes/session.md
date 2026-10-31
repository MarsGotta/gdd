---
summary: Adds the requests of your interactive Claude Code sessions to the log.
runs: your agents
---

## Who runs it

Your agent, in the [`gdd-setup`](../skills/setup.md), [`gdd-discover`](../skills/discover.md),
[`gdd-change`](../skills/change.md), [`gdd-learn`](../skills/learn.md) and
[`gdd-report`](../skills/report.md) skills, with the phase it worked in. An agent that gdd
started cannot run it: see [What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It lets [`gdd report`](report.md) count your conversations with the agent besides the calls
gdd made.

```bash
gdd session --phase 2-build --change cart-total
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Session imported: 0 new requests across 0 model(s), phase 2-build.
```

## See also

- [Measure the work](../../how-to/measure.md): what the log counts.
