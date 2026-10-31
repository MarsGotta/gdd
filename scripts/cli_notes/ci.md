---
summary: What CI runs: the harness unchanged, and G2 and G3 green, on the change's branch.
runs: CI
---

## Who runs it

CI: the workflow that [`gdd init`](init.md) writes, `.github/workflows/gdd.yml`, runs it on
every pull request and every push to `main` and `change/**`. You can run it to see what CI will
report.

## Examples

It writes nothing under `.gdd/`.

```bash
gdd ci
```

On the main branch there is no change to check:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Branch main: nothing to check per change.
```

On the branch of a change that edited a harness file and whose scenarios have no steps yet, it
lists each problem and exits with 1:

```text
gdd/rules.toml: harness path changed inside a change
G2 red: acceptance
G3 red: acceptance_full
CI failed
```

## See also

- [Run the gates in CI](../../how-to/ci.md): the workflow and what to add to it.
