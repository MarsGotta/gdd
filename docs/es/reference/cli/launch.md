# gdd launch

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/launch.md, nunca esta página.
-->

Runs the recipe of a change in its own git worktree, on its branch change/<id>, while this
checkout goes to the main branch: Build, G2, G3, the Reviewer, merge and pre; then it waits for
the human G4 when the change needs it, or goes to production. The worktree goes when the change
closes. Refuses uncommitted changes outside that branch, and a change past [parallel] max of
gdd/rules.toml changes building. Next to a change in flight whose planned paths overlap its
own, it warns first, also in its Decision Inbox: git settles the overlap when the change
merges. A change that parks exits with 1.

## Uso

```text
Usage: gdd launch <CHANGE>
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<CHANGE>` | Id of the change: its record is .gdd/changes/<id>/change.json. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

Your agent, as the Orchestrator, in the last step of the [`gdd-change`](../skills/change.md)
skill, once you approved the change. An agent that gdd started cannot run it: see [What an
agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It starts the Builder and the Reviewer, so each launch costs agent calls. It builds in a git
worktree of its own, on the branch `change/<id>`, and moves your checkout to the main branch.

```bash
gdd launch cart-total
```

It prints nothing while the agents work, and pauses at G4 when the change needs you. When the
change parks, it exits with 1. The [tutorial](../../getting-started/tutorial.md) shows a whole
launch with real output.

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Run a change](../../how-to/run-a-change.md): from the idea to production.
- [Run changes in parallel](../../how-to/parallel-changes.md): `[parallel] max` and overlaps.
- [Life of a change](../../explanation/life-of-a-change.md): every step of the recipe.
