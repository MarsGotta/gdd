---
summary: Records a gate decision: G0, the red zones, the plan, or G1 to G4 of a change.
runs: your agents
---

## Who runs it

Your agent, as the Orchestrator: G0 at the end of the [`gdd-setup`](../skills/setup.md) skill,
and G1, the red zones and the plan, with your OK, in the [`gdd-change`](../skills/change.md)
skill. [`gdd launch`](launch.md) opens G2 and G3 when they pass, and
[`gdd sitting`](sitting.md) records what you answer on its screen. An agent that gdd started
cannot run it: see [What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

Open G0 once the harness self-test passes:

```bash
gdd gate G0 ok --by machines
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
G0 ok (machines).
```

Approve the red zones:

```bash
gdd gate zones ok --by human --note "No sensitive folders yet"
```

```text
Red zones ok (2 red zone(s)).
```

G0 is never a person's decision:

```text
error: G0 is opened by the machines (--by machines): Brief written and harness 4/4.
```

## See also

- [Gates](../gates.md): what each gate checks and when it says no.
- [Write and approve scenarios](../../how-to/write-scenarios.md): G1 in practice.
