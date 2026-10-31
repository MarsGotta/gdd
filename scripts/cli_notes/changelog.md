---
summary: Shows what CHANGELOG.md lacks and, with --write, adds it.
runs: you, CI
---

## Who runs it

You, before a release, and CI, with `gdd changelog --check`, which fails when a line is
missing.

## Examples

It only adds lines: it never removes or rewrites a word of `CHANGELOG.md`. In a terminal it
asks before it writes.

```bash
gdd changelog
```

When nothing is missing, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
CHANGELOG.md covers every change on main.
```

## See also

- [Release versions and keep a changelog](../../how-to/releases-and-changelog.md): the whole
  path.
