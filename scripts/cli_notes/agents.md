---
summary: Writes and checks the GDD skills in the folder of each coding agent.
runs: you
---

## Who runs it

You. [`gdd init`](init.md) already writes the skills of the agents you chose.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

[`gdd agents check`](agents-check.md) checks that the skills are intact;
[`gdd agents install`](agents-install.md) writes them again.

```bash
gdd agents check
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true}
```

## See also

- [Agents and models](../../how-to/agents-and-models.md): add or change an agent.
