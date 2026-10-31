---
summary: Harness self-test: shows that the gates can say no.
runs: you, your agents
---

## Who runs it

Your agent, as the Orchestrator, in the [`gdd-setup`](../skills/setup.md) skill before it opens
G0, and you, after you change `gdd/commands.toml`. An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It breaks the harness on purpose and checks that the gates notice.

```bash
gdd selftest
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
OK a failing test turns G2 red
OK editing a protected file is blocked (Path Guard and CI)
OK rollback works in pre (simulated)
OK zero scenarios counts as a failure
harness 4/4
```

A Builder that tries it gets:

```text
error: Builders and Reviewers cannot run gdd selftest: an agent gdd started may only run gdd g2, gdd guard, gdd note, gdd status, gdd classify, gdd inbox, gdd report and gdd scope.
```

## See also

- [Gates](../gates.md#g0-setup-gate): G0 and its four checks.
