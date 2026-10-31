# gdd scope

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/scope.md, nunca esta página.
-->

Paths a change touched outside its plan: accepted when their color is not higher than the
change's, as JSON.

## Uso

```text
Usage: gdd scope <CHANGE>
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<CHANGE>` | Id of the change: its record is .gdd/changes/<id>/change.json. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

[`gdd g2`](g2.md), in build mode.

## Ejemplos

A path outside the plan is accepted when its color is not higher than the change's.

```bash
gdd scope cart-total
```

For a change that also touched a README, which is green, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true,"failures":[],"accepted":["README.md"]}
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [How risk picks the gates](../../explanation/risk.md): the colors of paths.
