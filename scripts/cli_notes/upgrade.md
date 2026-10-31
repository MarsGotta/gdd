---
summary: Proposes the files a newer gdd writes, on a branch gdd-upgrade.
runs: you
---

## Who runs it

You, after you install a new gdd, on the main branch with everything committed.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It proposes the files the new gdd writes, merged with your edits, on a branch `gdd-upgrade`.
Nothing is pushed: you review and merge the branch yourself.

```bash
gdd upgrade
```

When the project already has what this gdd writes, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Already up to date with gdd 0.1.0: nothing to propose.
```

## See also

- [Upgrade gdd](../../how-to/upgrade.md): the whole path, conflicts included.
