# gdd agents check

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/agents-check.md, nunca esta página.
-->

Checks that each GDD command and the instructions of each stack of the project of each agent
are exactly what install writes; names every file missing or edited, and exits with 1.

## Uso

```text
Usage: gdd agents check [OPTIONS]
```

## Opciones

| Opción | Descripción |
|---|---|
| `--agent <AGENTS>...` | The agents; without it, the integrations of gdd/models.toml. |

## Quién lo ejecuta

You, after you upgrade gdd or edit an agent's folder.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It names every file that is missing or edited, and then exits with 1.

```bash
gdd agents check
```

When every skill is intact, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":true}
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [`gdd agents`](agents.md): el comando al que pertenece este.
- [Agents and models](../../how-to/agents-and-models.md): add or change an agent.
- [Upgrade gdd in a project](../../how-to/upgrade.md): refresh the skills after an upgrade.
