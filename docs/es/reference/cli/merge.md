# gdd merge

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/merge.md, nunca esta página.
-->

Merges a change whose G3 is open into the main branch, with its record in the merge commit,
then builds the artifact and deploys it to pre. Prints the deploy as JSON.

## Uso

```text
Usage: gdd merge [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[CHANGE]` | Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

[`gdd launch`](launch.md), once G3 and the Reviewer pass; by hand, only to drive a change
without agents.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

```bash
gdd merge cart-total
```

It prints the deploy as JSON. The [tutorial](../../getting-started/tutorial.md) shows a real
merge.

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Life of a change](../../explanation/life-of-a-change.md): every step of the recipe.
