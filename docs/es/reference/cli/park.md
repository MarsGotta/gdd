# gdd park

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/park.md, nunca esta página.
-->

Parks a change: stops it with a message for the human in its Decision Inbox. Always exits with
1.

## Uso

```text
Usage: gdd park <CHANGE> <REASON>...
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<CHANGE>` | Id of the change: its record is .gdd/changes/<id>/change.json. |
| `<REASON>...` | Why; its words are joined with single spaces. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

[`gdd launch`](launch.md), when a gate stays red after its retries or an agent fails, and you,
to stop a change by hand.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It always exits with 1.

```bash
gdd park export-csv The CSV columns are not decided yet
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Change export-csv parked: The CSV columns are not decided yet. Message in .gdd/changes/export-csv/inbox.md
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Unblock a parked change](../../how-to/parked-change.md): read the message, then relaunch.
