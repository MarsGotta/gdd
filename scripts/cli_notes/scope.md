---
summary: Lists what a change touched outside its plan, and what is accepted, as JSON.
runs: your agents
---

## Who runs it

[`gdd g2`](g2.md), in build mode.

## Examples

A path outside the plan is accepted when its color is not higher than the change's.

```bash
gdd scope cart-total
```

For a change that also touched a README, which is green, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true,"failures":[],"accepted":["README.md"]}
```

## See also

- [How risk picks the gates](../../explanation/risk.md): the colors of paths.
