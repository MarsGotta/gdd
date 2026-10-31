---
summary: Runs a release step (pre, promote, observe), or releases the next version.
runs: you, your agents
---

## Who runs it

[`gdd launch`](launch.md) runs the steps; you run `gdd release` without a step.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

`pre` (tests and demo) runs after the merge, `promote` after G4 and `observe` after the
promotion. Without a step, it shows the next version of what main merged; `--yes` releases it.

```bash
gdd release
```

With nothing to release, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Nothing to release: no change on main carries a changelog line
```

The [tutorial](../../getting-started/tutorial.md) shows the steps of a real release.

## See also

- [Release versions and keep a changelog](../../how-to/releases-and-changelog.md): tags,
  `--yes` and `--version`.
