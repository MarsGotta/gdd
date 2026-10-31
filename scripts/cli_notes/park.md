---
summary: Stops a change with a message for you in its Decision Inbox.
runs: you, your agents
---

## Who runs it

[`gdd launch`](launch.md), when a gate stays red after its retries or an agent fails, and you,
to stop a change by hand.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It always exits with 1.

```bash
gdd park export-csv The CSV columns are not decided yet
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Change export-csv parked: The CSV columns are not decided yet. Message in .gdd/changes/export-csv/inbox.md
```

## See also

- [Unblock a parked change](../../how-to/parked-change.md): read the message, then relaunch.
