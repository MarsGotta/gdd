# Referencia de la CLI

La mayoría de estos comandos los ejecutan por ti tus agentes, con las
[skills de gdd](../skills/index.md); tú escribes pocos: `gdd init` una vez,
`gdd bootstrap` justo después, y luego `gdd board`
para ver tus Changes y `gdd sitting` cuando un Change te espera. De vez en cuando
ejecutas también `gdd status`, `gdd inbox`, `gdd report`, `gdd changelog` y
`gdd release`. Cada comando tiene una página con sus opciones, quién lo ejecuta y
ejemplos reales. La ayuda de gdd está en inglés, como la herramienta, y por ahora
también las notas de cada comando.

> [!NOTE]
> Generada por `scripts/make_reference.py` a partir de gdd 0.1.0: las opciones salen de
> `gdd <comando> --help` y el resto de cada página de `scripts/cli_notes/`. No edites
> estas páginas: cambia el texto de ayuda en el código o la nota y vuelve a ejecutar el
> script.

## Preparar un proyecto

| Comando | Qué hace | Quién lo ejecuta |
|---|---|---|
| [`gdd init`](init.md) | Sets up GDD in a folder: the harness, the Brief, the agents' skills, a stack skeleton. | Tú |
| [`gdd bootstrap`](bootstrap.md) | Runs the first install of each stack and commits its lock file. | Tú, tus agentes |
| [`gdd adopt`](adopt.md) | Records the checks a project with code already fails as known debt. | Tú, tus agentes |
| [`gdd agents`](agents.md) | Writes and checks the GDD skills in the folder of each coding agent. | Tú |
| [`gdd agents install`](agents-install.md) | Writes the GDD skills and the stack instructions for each agent. | Tú |
| [`gdd agents check`](agents-check.md) | Checks that each agent's GDD skills are exactly what gdd writes. | Tú |
| [`gdd stacks`](stacks.md) | Lists the stack sheets gdd comes with. | Tú |
| [`gdd stacks show`](stacks-show.md) | Shows one stack sheet: its tools, commands and protected paths. | Tú |
| [`gdd stacks compose`](stacks-compose.md) | Prints what several stack sheets give together, as TOML. | Tú |
| [`gdd upgrade`](upgrade.md) | Proposes the files a newer gdd writes, on a branch gdd-upgrade. | Tú |

## Planificar y aprobar

| Comando | Qué hace | Quién lo ejecuta |
|---|---|---|
| [`gdd new-change`](new-change.md) | Creates a change: its record and its branch change/<id>. | Tus agentes |
| [`gdd classify`](classify.md) | Prints the color, lane and gates of a change, as JSON. | Tus agentes |
| [`gdd sitting`](sitting.md) | Opens the sitting of a change: approve it before Build, or answer its G4. | Tú |
| [`gdd gate`](gate.md) | Records a gate decision: G0, the red zones, the plan, or G1 to G4 of a change. | Tus agentes |
| [`gdd note`](note.md) | Leaves a note on a change for its Decision Inbox. | Tus agentes |
| [`gdd changelog-line`](changelog-line.md) | Sets the changelog line a change's merge commit carries. | Tú |

## Construir y publicar

| Comando | Qué hace | Quién lo ejecuta |
|---|---|---|
| [`gdd launch`](launch.md) | Runs the recipe of a change: Build, G2, G3, the Reviewer, merge and pre. | Tus agentes |
| [`gdd resume`](resume.md) | Answers the G4 of a paused change: ok, ko or polish. | Tú, tus agentes |
| [`gdd park`](park.md) | Stops a change with a message for you in its Decision Inbox. | Tú, tus agentes |
| [`gdd prepare`](prepare.md) | Checks that a change may enter Build, and starts it. | Tus agentes |
| [`gdd merge`](merge.md) | Merges a change whose G3 is open into main, then deploys it to pre. | Tus agentes |
| [`gdd release`](release.md) | Runs a release step (pre, promote, observe), or releases the next version. | Tú, tus agentes |
| [`gdd changelog`](changelog.md) | Shows what CHANGELOG.md lacks and, with --write, adds it. | Tú, CI |

## Puertas y comprobaciones

| Comando | Qué hace | Quién lo ejecuta |
|---|---|---|
| [`gdd g2`](g2.md) | Loop Gate: the G2 commands, Path Guard and the scope, as JSON. | Tus agentes, CI |
| [`gdd g3`](g3.md) | Merge Gate: the G3 commands, Path Guard and the scenario count, as JSON. | Tus agentes, CI |
| [`gdd guard`](guard.md) | Path Guard: what a change touched that a role must not touch, as JSON. | Tus agentes |
| [`gdd snapshot`](snapshot.md) | Records what a change touched, so Path Guard judges only what comes after. | Tus agentes |
| [`gdd scope`](scope.md) | Lists what a change touched outside its plan, and what is accepted, as JSON. | Tus agentes |
| [`gdd selftest`](selftest.md) | Harness self-test: shows that the gates can say no. | Tú, tus agentes |
| [`gdd ci`](ci.md) | What CI runs: the harness unchanged, and G2 and G3 green, on the change's branch. | CI |

## Ver cómo va todo

| Comando | Qué hace | Quién lo ejecuta |
|---|---|---|
| [`gdd status`](status.md) | Where the project stands: G0, red zones, self-test, scenarios, changes and log. | Tú, tus agentes |
| [`gdd board`](board.md) | Shows the changes of the project by phase, on a read-only screen. | Tú |
| [`gdd inbox`](inbox.md) | The Decision Inbox of a change: what waits for you. | Tú |
| [`gdd report`](report.md) | Tokens, cost and time of the agent calls; with --time or --dora, the flow. | Tú, tus agentes |
| [`gdd learn`](learn.md) | Runs the Learn tools and lists the notes and findings that repeat. | Tú |
| [`gdd session`](session.md) | Adds the requests of your interactive Claude Code sessions to the log. | Tus agentes |

## Comportamiento común

### Opciones

| Opción | Qué hace |
|---|---|
| `-h`, `--help` | Imprime la ayuda de `gdd` o de un comando (`gdd gate --help`). La aceptan todos los comandos. |
| `-V`, `--version` | Imprime la versión de gdd: `gdd --version`. Solo la acepta `gdd`. |

`gdd` sin comando abre el [Board](board.md) cuando lo ejecuta una persona en un terminal;
sin terminal, o para un agente que gdd lanzó, imprime su ayuda. No hay más opciones
globales: cada comando lleva las suyas, detrás de su nombre.

### Dónde se ejecuta

Todos los comandos menos `gdd init` se ejecutan en un proyecto: la carpeta más cercana,
de la actual hacia arriba, que tiene `gdd/project.toml`. Fuera de uno, se para:

```text
error: not a GDD project: no gdd/project.toml in /home/me or the folders above it
```

`gdd init` es al revés: se niega dentro de un proyecto. Un comando que trabaja sobre un
Change recibe su id; donde su uso muestra `[CHANGE]`, el id puede venir de `GDD_CHANGE`
(consulta [Variables de entorno](../environment.md)).

### Salida

Los comandos imprimen texto para personas, salvo estos, que imprimen JSON en la salida
estándar para agentes y scripts: [`gdd agents install`](agents-install.md),
[`gdd new-change`](new-change.md), [`gdd classify`](classify.md), [`gdd prepare`](prepare.md),
[`gdd merge`](merge.md), [`gdd release`](release.md), [`gdd g2`](g2.md), [`gdd g3`](g3.md),
[`gdd guard`](guard.md), [`gdd scope`](scope.md). `gdd report` imprime JSON con `--json`.

Un error va a la salida de error, en una línea que empieza por `error:`, y nunca es
JSON. Una puerta que dice que no no es un error: `gdd g2`, `gdd g3` y `gdd guard`
imprimen `"ok": false` y salen con 0, salvo que les pases `--strict`.

### Códigos de salida

| Código | Significado |
|---|---|
| 0 | El comando hizo su trabajo. Un veredicto JSON aún puede decir `"ok": false`. |
| 1 | Un error, un comando u opción equivocados, o una comprobación que falló: `gdd selftest`, `gdd ci`, `gdd agents check`, `gdd changelog --check`, una puerta con `--strict`. `gdd park` siempre sale con 1, y también un `gdd launch` cuyo Change se aparca. |
| 2 | Una pantalla sin terminal: `gdd board`, `gdd sitting`, o `gdd init` sin sus flags. |

### Lo que los agentes no pueden ejecutar

gdd reconoce a un Builder o a un Reviewer por `GDD_ROLE`, que `gdd launch` fija para
cada agente que arranca. Estos comandos los rechazan, intente lo que intente el agente:
[`gdd upgrade`](upgrade.md), [`gdd gate`](gate.md), [`gdd changelog-line`](changelog-line.md),
[`gdd resume`](resume.md), [`gdd release`](release.md), y también `gdd selftest` y `gdd learn`.
Se paran con un mensaje como este:

```text
error: A Builder or Reviewer cannot open or close gates.
```

### Pantallas

Tres comandos abren una pantalla, y solo en una terminal: `gdd init` sin `--name`,
`--platform` y `--stack` abre el formulario de Setup, `gdd board` el Board y
`gdd sitting` la sentada de un Change. `gdd changelog` y `gdd release` sin paso
preguntan antes de escribir cuando se ejecutan en una terminal. Sin terminal, una
pantalla no hace nada, dice qué usar en su lugar y sale con 2:

```text
error: gdd board needs a terminal; gdd status lists the same changes
```

## Ver también

- [Configuración](../configuration.md): los ficheros de `gdd/` que leen estos comandos.
- [Variables de entorno](../environment.md): lo que gdd lee del entorno.
- [Ficheros que escribe gdd](../files.md): los registros, el log y la bandeja de `.gdd/`.
- [Tutorial](../../getting-started/tutorial.md): los comandos de un Change, con salida
  real.
