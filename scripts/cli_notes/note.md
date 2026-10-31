---
summary: Leaves a note on a change for its Decision Inbox.
runs: your agents
---

## Who runs it

Your agents, as the Orchestrator and the Builder, in their skills.

## Examples

`decided` records what they chose without you; `needs` records a decision that is yours, with
their recommendation. [`gdd inbox`](inbox.md) shows the notes.

```bash
gdd note cart-total decided "Totals round to two decimals"
gdd note cart-total needs "Should the total include taxes? I recommend no, taxes come later"
```

Each one prints:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
noted
```

## See also

- [Human stops](../../explanation/human-stops.md): when gdd waits for you.
