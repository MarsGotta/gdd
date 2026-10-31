---
summary: Records the checks a project with code already fails as known debt.
runs: you, your agents
---

## Who runs it

Your agent runs it once, in the [`gdd-setup`](../skills/setup.md) skill, in a repository that
already has code. You can also run it after [`gdd init`](init.md) and the first install. An
agent that gdd started cannot run it: see [What an agent may
run](../roles.md#what-an-agent-may-run).

## Examples

It records what already fails as known debt. From then on, a change that makes that debt
worse fails its gate.

```bash
gdd adopt
```

In a project whose code had an unused import, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, a Python project that already had code, in a real gdd-setup
     session. -->

```text
Passed: architecture, types, unit, dependencies, lockfile, security
Known debt: lint (1)
Proposed Refactors, riskiest first:
  1. debt-lint · Pay the known debt of lint · .gdd/adoption/lint.txt
Ask your agent for each one: /gdd-change pay the debt of <name> ($gdd-change in Codex).
```

A second run refuses:

```text
error: this project was already adopted on 2026-10-31 (gdd/debt.toml)
```

## See also

- [Bring gdd to an existing project](../../how-to/adopt-existing-project.md): the whole path.
