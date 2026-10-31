# gdd stacks compose

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/stacks-compose.md, nunca esta página.
-->

What the named sheets give together, as TOML: the shape of gdd/commands.toml plus a [protected]
table. Writes nothing.

## Uso

```text
Usage: gdd stacks compose [NAMES]...
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[NAMES]...` | The stacks; a name given twice counts once. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

You, for a project with more than one stack.

## Ejemplos

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

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [`gdd stacks`](stacks.md): el comando al que pertenece este.
- [Stacks](../stacks.md): what each sheet holds.
- [Gates and commands](../../how-to/gates-and-commands.md): change the commands of a gate.
