# gdd snapshot

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/snapshot.md, nunca esta página.
-->

Records what a change touched as it is now, so `gdd guard --since-snapshot` judges only what
changes after.

## Uso

```text
Usage: gdd snapshot [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[CHANGE]` | Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

[`gdd launch`](launch.md), after the Builder's test mode; by hand, only to drive a change
without agents.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

The change must be prepared first.

```bash
gdd snapshot cart-total
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true}
```

Before [`gdd prepare`](prepare.md), it refuses:

```text
error: change cart-total has not been prepared: its record has no build_start
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
