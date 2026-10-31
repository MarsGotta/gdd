---
summary: Shows one stack sheet: its tools, commands and protected paths.
runs: you
---

## Who runs it

You, before you choose a stack, or to compare it with your `gdd/commands.toml`.

## Examples

```bash
gdd stacks show python
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Python (python) · scenarios: pytest-bdd · detected by: pyproject.toml
Tools:
Lint + format (G2): Ruff (50.3k stars)
Types (G2): mypy (20.7k stars) · alternative: Pyright (15.7k stars)
Unit tests (G2): pytest (14.6k stars)
...
Commands:
install: uv sync --locked
bootstrap: uv lock && uv sync --locked
g2.lint: uv run ruff check . && uv run ruff format --check .
...
```

## See also

- [Stacks](../stacks.md): what each sheet holds.
- [Gates and commands](../../how-to/gates-and-commands.md): change the commands of a gate.
