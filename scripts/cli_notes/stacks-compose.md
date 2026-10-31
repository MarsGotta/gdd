---
summary: Prints what several stack sheets give together, as TOML.
runs: you
---

## Who runs it

You, for a project with more than one stack.

## Examples

It prints the `gdd/commands.toml` that the sheets give together, and writes nothing.

```bash
gdd stacks compose python
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
install = "uv sync --locked"

[g2]
lint = "uv run ruff check . && uv run ruff format --check ."
types = "uv run mypy src"
unit = "uv run pytest -q tests/unit"
acceptance = "uv run pytest -q tests/acceptance || [ $? -eq 5 ]"
architecture = "uv run lint-imports"

...
```

## See also

- [Stacks](../stacks.md): what each sheet holds.
- [Gates and commands](../../how-to/gates-and-commands.md): change the commands of a gate.
