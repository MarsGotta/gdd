# gdd report

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/report.md, nunca esta página.
-->

Tokens, cost and time of the agent calls in the log; with --time, where the time of each change
goes: active (agents, gates, release), waiting and between steps.

## Uso

```text
Usage: gdd report [OPTIONS]
```

## Opciones

| Opción | Descripción |
|---|---|
| `--by <BY>` | Fields to group the calls by, separated by commas (change, phase, model, role, agent...). Por defecto: `role`. |
| `--change <CHANGE>` | Count only the records of this change. |
| `--json` | Print the groups, or the changes with --time, as a JSON list; with --dora, one object. |
| `--time` | Where the time of each change goes, from its first line in the log to its last, instead of the agent calls; without --change, the project adds every change up. |
| `--dora` | DORA's five delivery metrics and GDD's signals, from the log and the records of the changes, instead of the agent calls. |
| `--since <SINCE>` | With --time or --dora, only the changes created on or after this day, YYYY-MM-DD (UTC). |

## Quién lo ejecuta

You, or your agent in the [`gdd-report`](../skills/report.md) and
[`gdd-learn`](../skills/learn.md) skills.

## Ejemplos

```bash
gdd report --time
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
cart-total · Show the cart total · in progress
  Flow time 26s · active 4s (14%) · waiting 15s · between steps 7s
  Gates: security 2s, secrets 1s, acceptance 0s, acceptance_full 0s, unit 0s, architecture 0s, dependencies 0s, lint 0s, types 0s, lockfile 0s, path-guard 0s
  Waiting: to start 15s

Project: 1 change(s) · flow time 26s · active 4s (14%) · waiting 15s · between steps 7s
Biggest waits: to start 15s
```

Before any agent call:

```text
No agent calls in the log yet.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Measure the work](../../how-to/measure.md): every report and what to read in it.
- [Measuring](../../explanation/measuring.md): why these numbers.
