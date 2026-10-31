# gdd prepare

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/prepare.md, nunca esta página.
-->

Checks that a change may enter Build (G0, red zones, G1, plan, model families, its branch) and
starts it. Prints its decision, with the model of each role, as JSON.

## Uso

```text
Usage: gdd prepare [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[CHANGE]` | Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

[`gdd launch`](launch.md), as the first step of its recipe; by hand, only to drive a change
without agents.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It runs on the branch `change/<id>` and checks G0, the red zones, G1, the plan, the model
families and the branch.

On the branch `change/cart-total`:

```bash
gdd prepare cart-total
```

You should see its decision, with the model of each role:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true,"change":"cart-total","color":"yellow","lane":"full",...}
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Life of a change](../../explanation/life-of-a-change.md): every step of the recipe.
