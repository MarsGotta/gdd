# gdd session

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/session.md, nunca esta página.
-->

Adds the requests of the interactive Claude Code sessions of the project to the log, one line
per model and each request once; the sessions gdd started for its agents are left out.

## Uso

```text
Usage: gdd session [OPTIONS] --phase <PHASE>
```

## Opciones

| Opción | Descripción |
|---|---|
| `--phase <PHASE>` | The phase of the method the conversation belongs to. Valores posibles: `0-setup`, `1-discover`, `2-build`, `3-integrate`, `4-release`, `5-learn`. |
| `--change <CHANGE>` | Count it for this change. |

## Quién lo ejecuta

Your agent, in the [`gdd-setup`](../skills/setup.md), [`gdd-discover`](../skills/discover.md),
[`gdd-change`](../skills/change.md), [`gdd-learn`](../skills/learn.md) and
[`gdd-report`](../skills/report.md) skills, with the phase it worked in. An agent that gdd
started cannot run it: see [What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It lets [`gdd report`](report.md) count your conversations with the agent besides the calls
gdd made.

```bash
gdd session --phase 2-build --change cart-total
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Session imported: 0 new requests across 0 model(s), phase 2-build.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Measure the work](../../how-to/measure.md): what the log counts.
