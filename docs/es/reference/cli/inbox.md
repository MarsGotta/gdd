# gdd inbox

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/inbox.md, nunca esta página.
-->

The Decision Inbox of a change: what is open for the human. Also saved in
.gdd/changes/<id>/inbox.md.

## Uso

```text
Usage: gdd inbox [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[CHANGE]` | Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE. |

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

You, when a change needs you or has parked.

## Ejemplos

Your agents write to it with [`gdd note`](note.md); gdd also keeps it in
`.gdd/changes/<id>/inbox.md`.

```bash
gdd inbox cart-total
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
# Decision Inbox · cart-total · Show the cart total

Color ? · lane ? · state open

## Needs your decision
- Should the total include taxes? I recommend no, taxes come later

## Decided without you
- Totals round to two decimals

## Parked
- Nothing.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Unblock a parked change](../../how-to/parked-change.md): what to do with a parked inbox.
