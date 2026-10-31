# Skills

Una skill de gdd es un conjunto de instrucciones que `gdd init` instala en tu agente de código.
Ejecutas una skill en una conversación con tu agente, y el agente la sigue paso a paso y ejecuta
por ti los comandos de gdd. Esta página enumera todas las skills, dónde viven y cómo se llaman.
Consúltala para encontrar la skill del paso en que estás.

## Antes de empezar

- Un proyecto preparado con [`gdd init`](../cli/init.md). Instala las skills para cada agente de
  código que hayas elegido.
- Tu agente de código, arrancado en la carpeta del proyecto, en `main`.

## Dónde viven las skills

Cada agente guarda las skills en su propia carpeta, con los nombres `gdd-<name>`:

| Agente | Ficheros que escribe `gdd init` | Cómo llamas a una skill |
|---|---|---|
| Claude Code | `.claude/skills/gdd-<name>/SKILL.md` | `/gdd-change` |
| Codex | `.agents/skills/gdd-<name>/SKILL.md` | `$gdd-change` |
| OpenCode | `.opencode/commands/gdd-<name>.md` | `/gdd-change` |

Cada fichero empieza con una línea que dice que es generado: lo que edites ahí se pierde.
[`gdd agents install`](../cli/agents-install.md) los vuelve a escribir, por ejemplo después de
cambiar `gdd/models.toml` o de actualizar gdd, y [`gdd agents check`](../cli/agents-check.md)
dice si alguno se ha editado o falta.

Las carpetas forman parte del [arnés](../../glossary.md#arnés), los ficheros que usa gdd para
comprobar tu proyecto, que es una [zona roja](../../glossary.md#zona-roja): revisa esos ficheros
y haz su commit en `main`.
[Elegir agentes y modelos](../../how-to/agents-and-models.md#5-instala-los-comandos-de-gdd-en-cada-agente)
muestra los dos comandos con su salida.

## Cómo llamar a una skill

Escribe su nombre en tu agente, seguido de lo que quieres, con tus propias palabras. Las palabras
que siguen al nombre son la entrada de la skill. En Claude Code y OpenCode:

```text
/gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

En Codex:

```text
$gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

La entrada es opcional en todas las skills: `/gdd-status` a secas es una llamada completa. El
agente hace de [Orchestrator](../roles.md#el-orchestrator) y te habla en el idioma de
`language` en `gdd/project.toml`. Las skills se citan unas a otras con la sintaxis del propio
agente: en Codex, `gdd-status` propone `$gdd-change`; en Claude Code, `/gdd-change`.

## Las skills que ejecutas tú

| Skill | Ejecútala para | Cuándo |
|---|---|---|
| [`gdd-setup`](setup.md) | escribir el Brief del proyecto, elegir el ejecutor de escenarios y dejar listo el arnés, para que las máquinas abran la puerta de preparación (G0, *Setup Gate*) | una vez por proyecto, justo después de `gdd init` |
| [`gdd-discover`](discover.md) | explorar contigo la siguiente pieza de valor: opciones, Spikes, decisiones y el orden de los próximos Changes | después de Setup, y siempre que el siguiente paso no sea un único Change pequeño |
| [`gdd-change`](change.md) | preparar un Change, pedir tu OK en una sola sentada, lanzarlo y traértelo de vuelta en la puerta de salida (G4, *Release Gate*) | cada vez que quieras construir, arreglar o reorganizar algo |
| [`gdd-status`](status.md) | saber en qué punto está el proyecto y cuál es el siguiente paso | cuando vuelves a un proyecto, o no sabes qué viene después |
| [`gdd-report`](report.md) | leer cuánto han costado las llamadas de agente, en tokens y en dinero | cuando quieres los números |
| [`gdd-learn`](learn.md) | encontrar los errores que se repiten y proponer una regla o una comprobación para cada uno | de vez en cuando, después de varios Changes |
| [`gdd-hotfix`](hotfix.md) | revertir producción primero y después arreglar la causa en un Change propio | cuando algo está roto en producción |

El orden habitual es `gdd-setup` y después `gdd-change` para cada Change. Ejecuta
`gdd-discover` antes de un Change cuando el producto necesita primero una arquitectura o un mapa
de sus áreas sensibles. `gdd-status` es la forma de volver en cualquier momento: siempre termina
con un siguiente paso.

## Las skills que usan los agentes de gdd

Cuando gdd ejecuta un Change, lanza agentes él mismo, sin conversación, y entrega a cada uno una
de estas skills. Tú nunca las ejecutas: esperan una entrada que solo escribe gdd.

| Skill | Quién la recibe | Qué hace |
|---|---|---|
| [`gdd-build-test`](agent-skills.md#gdd-build-test) | el Builder en modo test | escribe los pasos de los escenarios y los tests, que fallan porque falta el comportamiento |
| [`gdd-build`](agent-skills.md#gdd-build) | el Builder en modo build | escribe el código hasta que pasa la puerta del bucle (G2, *Loop Gate*) |
| [`gdd-review`](agent-skills.md#gdd-review) | el Reviewer | lee el Change entero y responde OK o KO |
| [`gdd-replan`](agent-skills.md#gdd-replan) | el Orchestrator, desatendido | averigua por qué G2 sigue en rojo tras los reintentos del Builder y escribe un enfoque nuevo |

gdd también escribe una skill `gdd-stack-<name>` por cada stack de `gdd/project.toml`, como
`gdd-stack-python`: las instrucciones propias del stack, que esos agentes leen antes de
trabajar. [Skills de los agentes de gdd](agent-skills.md) tiene los detalles.

## Ver también

- [Describe un Change a tu agente](../../how-to/run-a-change.md): la skill `gdd-change` en uso.
- [Roles](../roles.md): qué pueden y qué no pueden hacer el Orchestrator, el Builder y el
  Reviewer.
- [Referencia de la CLI](../cli/index.md): los comandos de gdd que ejecutan las skills.
- [Pantallas](../screens.md): la sentada y el Board, si prefieres una pantalla al chat.
