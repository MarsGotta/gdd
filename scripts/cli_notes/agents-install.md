---
summary: Writes the GDD skills and the stack instructions for each agent.
runs: you
---

## Who runs it

You, when you add an agent to the project or when
[`gdd agents check`](agents-check.md) reports a missing or edited skill.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It writes the skill files and nothing else.

Write the skills of Claude Code only:

```bash
gdd agents install --agent claude
```

You should see the paths it wrote, as JSON:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true,"written":[".claude/skills/gdd-build/SKILL.md",".claude/skills/gdd-build-test/SKILL.md",...,".claude/skills/gdd-stack-python/SKILL.md"]}
```

## See also

- [Agents and models](../../how-to/agents-and-models.md): add or change an agent.
