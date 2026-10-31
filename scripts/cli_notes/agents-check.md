---
summary: Checks that each agent's GDD skills are exactly what gdd writes.
runs: you
---

## Who runs it

You, after you upgrade gdd or edit an agent's folder.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It names every file that is missing or edited, and then exits with 1.

```bash
gdd agents check
```

When every skill is intact, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true}
```

## See also

- [Agents and models](../../how-to/agents-and-models.md): add or change an agent.
- [Upgrade gdd in a project](../../how-to/upgrade.md): refresh the skills after an upgrade.
