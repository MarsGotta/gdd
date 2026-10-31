# gdd new-change

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/new-change.md, nunca esta página.
-->

Creates a change: its record and, when it has none, its branch change/<id> from the main
branch. Logs it and prints its classification as JSON.

## Uso

```text
Usage: gdd new-change [OPTIONS] --title <TITLE> <CHANGE>
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<CHANGE>` | Id of the change: lowercase letters, digits and hyphens. |

## Opciones

| Opción | Descripción |
|---|---|
| `--title <TITLE>` | What the change is about, in a few words. |
| `--type <KIND>` | The kind of work. Por defecto: `Feature`. Valores posibles: `Feature`, `Hotfix`, `Refactor`, `Migration`, `Spike`, `No-code`. |
| `--paths [<PATHS>...]` | Paths the change plans to touch; patterns are allowed. |
| `--resumable` | The change can be resumed after an interruption (a condition of the fast lane). |
| `--min-color <MIN_COLOR>` | Declared minimum color: it raises the color, never lowers it. |
| `--option <OPTION>` | One variant of a Spike for this option of its question; once for each option (2 to 4). |
| `--variants <N>` | That many variants of a Spike on the same brief (2 to 4). |

## Quién lo ejecuta

Your agent, as the Orchestrator, in the [`gdd-change`](../skills/change.md) skill, and in the
[`gdd-discover`](../skills/discover.md) skill for Spikes.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

```bash
gdd new-change cart-total --title "Show the cart total" \
  --paths src/shop/cart.py tests/unit/test_cart.py
```

You should see how gdd classified the change:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{
  "change": "cart-total",
  "color": "yellow",
  "lane": "full",
  "reasons": [
    "src/shop/cart.py -> yellow (no zone: default color)",
    "tests/unit/test_cart.py -> yellow (no zone: default color)"
  ],
  "g1": false,
...
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Run a change](../../how-to/run-a-change.md): from the idea to production.
- [Spikes](../../how-to/spikes.md): `--type Spike`, `--option` and `--variants`.
