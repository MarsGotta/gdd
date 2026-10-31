# gdd agents

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/agents.md, nunca esta página.
-->

The GDD commands, and the instructions of each stack in `stack` of gdd/project.toml, in the
folders of the coding agents (claude, codex, opencode), as their agent sheets say.

## Uso

```text
Usage: gdd agents <COMMAND>
```

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Subcomandos

| Subcomando | Descripción |
|---|---|
| [`gdd agents install`](agents-install.md) | Writes every GDD command and the instructions of each stack of the project for each agent, and no other file. Prints the paths as JSON. |
| [`gdd agents check`](agents-check.md) | Checks that each GDD command and the instructions of each stack of the project of each agent are exactly what install writes; names every file missing or edited, and exits with 1. |

## Quién lo ejecuta

You. [`gdd init`](init.md) already writes the skills of the agents you chose.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

[`gdd agents check`](agents-check.md) checks that the skills are intact;
[`gdd agents install`](agents-install.md) writes them again.

```bash
gdd agents check
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true}
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Agents and models](../../how-to/agents-and-models.md): add or change an agent.
