---
summary: Runs the Learn tools and lists the notes and findings that repeat.
runs: you
---

## Who runs it

You, from time to time.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

It runs the Learn tools and lists what repeats across changes. It is never a gate: it exits
with 0 whatever its tools found.

```bash
gdd learn
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
duplication: nothing found
...
unused: found something (exit code 3)
  src/shop/greeting.py:4: unused function 'greeting' (60% confidence)
  All it said: .gdd/learn/unused.txt

Polish notes at G4 (0)

Reviewer findings that repeat: none
Reviews whose findings name no class: none
```

## See also

- [Measure the work](../../how-to/measure.md): Learn in practice.
