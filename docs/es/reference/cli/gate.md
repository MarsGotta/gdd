# gdd gate

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/gate.md, nunca esta página.
-->

Records a gate decision: G0 (by the machines), the red zones (by a human), or the plan, G1, G2,
G3 or G4 of a change. Approving G1 seals the scenario files of the change. Builders and
Reviewers cannot record anything.

## Uso

```text
Usage: gdd gate [OPTIONS] <GATE> <DECISION> [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<GATE>` | The gate. Valores posibles: `G0`, `zones`, `plan`, `G1`, `G2`, `G3`, `G4`. |
| `<DECISION>` | ok or ko; approve and reject say the same. |
| `[CHANGE]` | Id of the change, for the plan, G1, G2, G3 and G4. |

## Opciones

| Opción | Descripción |
|---|---|
| `--by <BY>` | Who decides: human or machines. Por defecto: `human`. |
| `--note <NOTE>` | What to read about the decision. Por defecto: `""`. |

## Quién lo ejecuta

Your agent, as the Orchestrator: G0 at the end of the [`gdd-setup`](../skills/setup.md) skill,
and G1, the red zones and the plan, with your OK, in the [`gdd-change`](../skills/change.md)
skill. [`gdd launch`](launch.md) opens G2 and G3 when they pass, and
[`gdd sitting`](sitting.md) records what you answer on its screen. An agent that gdd started
cannot run it: see [What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

Open G0 once the harness self-test passes:

```bash
gdd gate G0 ok --by machines
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
G0 ok (machines).
```

Approve the red zones:

```bash
gdd gate zones ok --by human --note "No sensitive folders yet"
```

```text
Red zones ok (2 red zone(s)).
```

G0 is never a person's decision:

```text
error: G0 is opened by the machines (--by machines): Brief written and harness 4/4.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Gates](../gates.md): what each gate checks and when it says no.
- [Write and approve scenarios](../../how-to/write-scenarios.md): G1 in practice.
