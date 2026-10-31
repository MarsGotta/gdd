# gdd report

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.1. Edita el texto de ayuda en el
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

<!-- REAL OUTPUT: gdd 0.1.1, sandbox project shop (python; claude and codex). -->

```text
cart-total · Show the cart total · in progress
  Flow time 34s · active 2s (7%) · waiting 14s · between steps 18s
  Gates: security 1s, secrets 0s, unit 0s, acceptance 0s, acceptance_full 0s, architecture 0s, lint 0s, types 0s, dependencies 0s, lockfile 0s, path-guard 0s
  Waiting: to start 14s

Project: 1 change(s) · flow time 34s · active 2s (7%) · waiting 14s · between steps 18s
Biggest waits: to start 14s
```

By role, for a change whose Reviewer ran on Codex with no price for its model in
`gdd/models.toml`:

```bash
gdd report --by role --change cart-total
```

<!-- REAL OUTPUT: gdd 0.1.1, with stand-in agents: the acceptance harness's fake claude and
     codex scripts, so the tokens and costs are theirs, not a real model's. -->

```text
role -> calls · tokens in · cache · out · cost USD · seconds
builder-build -> 1 · 100 · 50 · 20 · 0.0100 · 0
builder-test -> 1 · 100 · 50 · 20 · 0.0100 · 0
reviewer -> 1 · 50 · 50 · 25 · unknown · 0
TOTAL -> in 250 · cache 150 · out 65 · 0.0200 USD + unknown (1 call(s) with no price)
unknown: no price in gdd/models.toml for gpt-6
Human stops logged: 1
```

Codex reports no cost, so the Reviewer's is `unknown`, never 0. With a `[prices.gpt-6]` row it
would be an estimate, marked `~`.

Before any agent call:

```text
No agent calls in the log yet.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Measure the work](../../how-to/measure.md): every report and what to read in it.
- [Measuring](../../explanation/measuring.md): why these numbers.
