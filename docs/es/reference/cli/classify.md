# gdd classify

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/classify.md, nunca esta página.
-->

Color, lane and gates of a change, from its plan and what it touched, as JSON.

## Uso

```text
Usage: gdd classify [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[CHANGE]` | Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

Your agent, as the Orchestrator in the [`gdd-replan`](../skills/agent-skills.md) skill, after
a replan.

## Ejemplos

gdd also classifies a change when it creates it and when it prepares it.

```bash
gdd classify cart-total
```

You should see the color, the lane and the reason for each path:

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
  "features": [],
...
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [How risk picks the gates](../../explanation/risk.md): where the color comes from.
