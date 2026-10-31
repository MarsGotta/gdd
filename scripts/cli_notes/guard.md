---
summary: Path Guard: what a change touched that a role must not touch, as JSON.
runs: your agents
---

## Who runs it

Your agent, as the Builder, in the [`gdd-build-test`](../skills/agent-skills.md) and
[`gdd-build`](../skills/agent-skills.md) skills, before G2. G2 and G3 also run Path Guard.

## Examples

The Builder in test mode may not touch production code:

```bash
gdd guard cart-total --role builder-test
```

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":false,"failures":["src/shop/cart.py: protected path for builder-test"]}
```

With `--since-snapshot`, it judges only what changed since the last
[`gdd snapshot`](snapshot.md):

```bash
gdd guard cart-total --role builder-test --since-snapshot
```

```text
{"ok":true,"failures":[]}
```

## See also

- [Roles](../roles.md): what each role may touch.
- [Safety](../../explanation/safety.md): why Path Guard exists.
