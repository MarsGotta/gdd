# gdd stacks show

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/stacks-show.md, nunca esta página.
-->

One sheet: the tool for each need, the commands and the protected paths.

## Uso

```text
Usage: gdd stacks show <NAME>
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<NAME>` | The stack. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

You, before you choose a stack, or to compare it with your `gdd/commands.toml`.

## Ejemplos

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

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [`gdd stacks`](stacks.md): el comando al que pertenece este.
- [Stacks](../stacks.md): what each sheet holds.
- [Gates and commands](../../how-to/gates-and-commands.md): change the commands of a gate.
