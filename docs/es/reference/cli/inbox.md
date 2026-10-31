# gdd inbox

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.1. Edita el texto de ayuda en el
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

<!-- REAL OUTPUT: gdd 0.1.1, sandbox project shop (python; claude and codex). -->

```text
# Decision Inbox · cart-total · Show the cart total

Color and lane not decided yet (gdd decides them at launch) · state open

## Needs your decision
- Should the total include taxes? I recommend no, taxes come later

## Decided without you
- Totals round to two decimals

## Parked
- Nothing.
```

A KO you gave stays at the top, under `## Your decisions`, until that gate is decided again.
After `gdd gate G1 ko export-csv --by human --note "Say which columns the file has"`:

```text
# Decision Inbox · export-csv · Export the catalog as CSV

Color and lane not decided yet (gdd decides them at launch) · state open

## Your decisions
- G1 ko: Say which columns the file has

## Needs your decision
- Nothing.

## Decided without you
- Nothing.

## Parked
- Nothing.
```

At G4 it asks you to answer your agent, or the sitting screen:

<!-- REAL OUTPUT: gdd 0.1.1, with stand-in agents: the acceptance harness's fake claude and
     codex scripts, so the tokens and costs are theirs, not a real model's. -->

```text
# Decision Inbox · cart-total · Show the cart total

Color yellow · lane full · state in-pre

## How it works
- The Reviewer did not explain the change.

## Needs your decision
- G4 · Release Gate: check the pre demo and the change message, then answer your agent (or the sitting screen): OK promotes to production, KO rejects it (production untouched), polish with your notes runs one more Builder round. Your agent runs gdd resume cart-total --g4 ok, --g4 ko or --g4 polish --note <your notes>.

## Decided without you
- Paths outside the package, same or lower color, accepted by rule: tests/steps/app.txt

## Parked
- Nothing.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Unblock a parked change](../../how-to/parked-change.md): what to do with a parked inbox.
