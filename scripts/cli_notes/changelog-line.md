---
summary: Sets the changelog line a change's merge commit carries.
runs: you
---

## Who runs it

You, before the change merges.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It sets the line that replaces the change's title in the changelog; `--none` leaves the change
out.

```bash
gdd changelog-line cart-total Added "The cart shows its total"
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Changelog line of cart-total: Added · The cart shows its total
```

## See also

- [Release versions and keep a changelog](../../how-to/releases-and-changelog.md): sections,
  `--breaking` and `--none`.
