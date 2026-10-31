# gdd agents install

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/agents-install.md, nunca esta página.
-->

Writes every GDD command and the instructions of each stack of the project for each agent, and
no other file. Prints the paths as JSON.

## Uso

```text
Usage: gdd agents install [OPTIONS]
```

## Opciones

| Opción | Descripción |
|---|---|
| `--agent <AGENTS>...` | The agents; without it, the integrations of gdd/models.toml. |

## Quién lo ejecuta

You, when you add an agent to the project or when
[`gdd agents check`](agents-check.md) reports a missing or edited skill.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It writes the skill files and nothing else.

Write the skills of Claude Code only:

```bash
gdd agents install --agent claude
```

You should see the paths it wrote, as JSON:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true,"written":[".claude/skills/gdd-build/SKILL.md",".claude/skills/gdd-build-test/SKILL.md",...,".claude/skills/gdd-stack-python/SKILL.md"]}
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [`gdd agents`](agents.md): el comando al que pertenece este.
- [Agents and models](../../how-to/agents-and-models.md): add or change an agent.
