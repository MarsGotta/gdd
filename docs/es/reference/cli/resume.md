# gdd resume

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/resume.md, nunca esta página.
-->

Answers the G4 of a change paused at it: ok takes it to production, ko rejects it, and polish
runs one more Builder round with your notes and pauses at G4 again. Builders and Reviewers
cannot answer.

## Uso

```text
Usage: gdd resume [OPTIONS] --g4 <G4> <CHANGE>
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<CHANGE>` | Id of the change: its record is .gdd/changes/<id>/change.json. |

## Opciones

| Opción | Descripción |
|---|---|
| `--g4 <G4>` | The answer to G4. Valores posibles: `ok`, `ko`, `polish`. |
| `--note <NOTE>...` | What to read about the answer: the rest of the words. Polish needs it. |
| `--variant <N>` | The variant of a Spike with variants that OK brings to main, or that polish builds again. |

## Quién lo ejecuta

Your agent, as the Orchestrator, with your answer, in the [`gdd-change`](../skills/change.md)
skill; or you, on the G4 page of [`gdd sitting`](sitting.md), which runs the same answer. An
agent that gdd started cannot run it: see [What an agent may
run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

```bash
gdd resume cart-total --g4 ok
gdd resume cart-total --g4 polish --note "Show the total with its currency"
```

`ok` takes the change to production, `ko` rejects it, and `polish` runs one more Builder round
with your note, then pauses at G4 again. The [tutorial](../../getting-started/tutorial.md)
shows a real G4 answer.

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Answer the Release Gate](../../how-to/release-gate.md): what to look for at G4.
