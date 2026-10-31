# gdd note

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/note.md, nunca esta página.
-->

Leaves a note on a change for its Decision Inbox.

## Uso

```text
Usage: gdd note <CHANGE> <KIND> <TEXT>...
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<CHANGE>` | Id of the change: its record is .gdd/changes/<id>/change.json. |
| `<KIND>` | needs (a decision for the human), decided (taken without them) or parked. Valores posibles: `needs`, `decided`, `parked`. |
| `<TEXT>...` | What the note says; its words are joined with single spaces. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

Your agents, as the Orchestrator and the Builder, in their skills.

## Ejemplos

`decided` records what they chose without you; `needs` records a decision that is yours, with
their recommendation. [`gdd inbox`](inbox.md) shows the notes.

```bash
gdd note cart-total decided "Totals round to two decimals"
gdd note cart-total needs "Should the total include taxes? I recommend no, taxes come later"
```

Each one prints:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
noted
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Human stops](../../explanation/human-stops.md): when gdd waits for you.
