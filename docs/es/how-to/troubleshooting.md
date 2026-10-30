# Solución de problemas

Cada entrada parte de lo que ves (un mensaje de gdd o un Change que se detiene) y luego da la
causa y la solución. Los mensajes se citan tal como los imprime gdd, en inglés; `CHANGE`
representa el id de tu Change, y los números, rutas e ids que aparecen en ellos son ejemplos.
Si un Change se ha aparcado, empieza por [Vuelve a poner en marcha un Change
aparcado](parked-change.md).

## Antes de empezar

- Ejecuta primero `bin/gdd status`: te dice cómo están G0, las zonas rojas, el autotest y cada
  Change.
- Para un Change concreto, `bin/gdd inbox CHANGE` muestra su Decision Inbox, y
  `.gdd/changes/CHANGE/` guarda los informes de las puertas (`g2.json`, `g3.json`) y lo que
  dijo cada agente (`outputs/`).

## Encuentra tu mensaje

| Cuándo | Entradas |
|---|---|
| Antes de que empiece el build | [G0 pendiente](#g0-pendiente-ejecuta-el-setup) · [G0 no se puede abrir](#g0-no-se-puede-abrir) · [Autotest: G2 en rojo](#autotest-g2-está-en-rojo-sin-el-test-que-falla) · [Zonas rojas sin aprobar](#zonas-rojas-aún-sin-aprobar) · [Zonas rojas cambiadas](#zonas-rojas-cambiadas-después-de-aprobarse) · [Escenarios sin G1](#escenarios-nuevos-o-cambiados-sin-g1) · [Plan de un Change rojo](#change-rojo-una-persona-debe-aprobar-el-plan) · [Misma familia de modelos](#reviewer-y-builder-son-de-la-misma-familia-de-modelos) · [Agente no encontrado](#no-se-encuentra-el-programa-de-un-agente-en-el-path) · [Cambios sin commit](#el-árbol-de-trabajo-tiene-cambios-sin-commit) · [No es el checkout principal](#ejecuta-gdd-launch-desde-el-checkout-principal) · [El tope](#se-ha-alcanzado-el-tope-de-changes-en-construcción) |
| En una sentada | [G1 rechazada](#g1-rechazada-los-ejemplos-ocultos-usan-frases-de-paso-desconocidas) · [Sin terminal](#gdd-sitting-necesita-un-terminal) · [El Builder no puede registrar](#un-builder-o-un-reviewer-no-puede-abrir-ni-cerrar-puertas) |
| Durante el build | [G2 sigue en rojo](#aparcado-g2-sigue-en-rojo-tras-3-reintentos-y-1-replanificación) · [Ruta protegida](#ruta-protegida-para-builder-build) · [Los comandos no se ejecutaron](#los-comandos-de-la-puerta-no-se-ejecutaron-primero-debe-pasar-path-guard) · [Escenario aprobado modificado](#escenario-aprobado-en-g1-y-modificado-después) · [Fuera del paquete](#fuera-del-paquete-y-rojo) · [El modo test escribió código](#path-guard-ha-fallado-tras-el-modo-test) · [El agente cambió el estado de GDD](#cambio-en-el-estado-de-gdd) · [El agente no respondió](#no-respondió-en-n-segundos) · [Presupuesto del Spike](#el-spike-ha-agotado-su-presupuesto) |
| En G2 y G3 | [Comando que agota el tiempo](#tiempo-agotado-sube-limitscommand_seconds) · [Timeouts del mutation testing](#el-mutation-testing-agota-el-tiempo) · [Sin configurar](#gddcommandstoml-no-tiene-comandos-para-esta-puerta) · [Cero escenarios](#cero-escenarios-cuenta-como-fallo) |
| En la fusión | [Conflictos con main](#main-ha-avanzado-entretanto-y-la-fusión-tiene-conflictos) · [G3 en rojo sobre main](#g3-en-rojo-sobre-el-último-main) · [KO del Reviewer](#ko-del-reviewer-o-g3-en-rojo-tras-una-corrección) · [Checkout principal con cambios](#la-fusión-necesita-el-checkout-principal-sin-cambios-pendientes) |
| En G4 | [No está en pausa en G4](#no-está-en-pausa-en-g4) · [Elige una variante](#elige-una-variante) |

## G0 pendiente: ejecuta el Setup

```text
error: G0 pending: run the Setup (the gdd-setup command).
```

**Causa.** Ningún Change empieza antes de que la puerta de preparación (G0, *Setup Gate*) esté
abierta. `bin/gdd status` muestra `G0: pending`.

**Solución.** Arranca tu agente y ejecuta el comando `gdd-setup` (`/gdd-setup` en Claude Code y
OpenCode, `$gdd-setup` en Codex). Escribe el Brief, ejecuta `bin/gdd selftest` y abre G0 con
`bin/gdd gate G0 ok --by machines`.

## G0 no se puede abrir

```text
error: G0 cannot open: the harness self-test is not 4/4 (bin/gdd selftest).
error: G0 cannot open: docs/brief.md (Project Brief) is missing.
error: G0 is opened by the machines (--by machines): Brief written and harness 4/4.
```

**Causa.** G0 la abren las máquinas cuando existe el Project Brief y el último autotest pasó
las cuatro comprobaciones, y nadie más puede abrirla.

**Solución.** Escribe `docs/brief.md`, ejecuta `bin/gdd selftest` hasta que pase y luego
ejecuta `bin/gdd gate G0 ok --by machines`. El resultado del autotest está en
`.gdd/selftest.json`.

## Autotest: G2 está en rojo sin el test que falla

```text
KO a failing test turns G2 red (G2 is red without it: lint)
KO a failing test turns G2 red (gdd/debt.toml: invalid TOML: TOML parse error at line 17, column 1
```

**Causa.** La primera comprobación del autotest necesita G2 en verde antes de plantar su test
que falla, y juzga G2 igual que lo hace G2: un comando que se mantiene dentro de su deuda
conocida en `gdd/debt.toml` no pone G2 en rojo. El comando nombrado falla en main sin deuda
conocida, o da más problemas de los que marca su recuento. En la segunda forma del mensaje,
`gdd/debt.toml` no se puede leer, y a continuación va el motivo.

**Solución.** Haz que el comando nombrado pase en main o, en un proyecto que ya tiene código,
registra lo que falla hoy con `bin/gdd adopt` (consulta [Lleva gdd a un proyecto
existente](adopt-existing-project.md#4-registra-la-deuda-conocida-con-gdd-adopt)). Si un comando
supera su recuento, tienes que corregir sus problemas nuevos. Si `gdd/debt.toml` no se puede
leer, corrígelo. Después vuelve a ejecutar `bin/gdd selftest`.

## Zonas rojas aún sin aprobar

```text
error: Red zones not approved yet: the Orchestrator proposes them at the end of Discover. (bin/gdd gate zones ok --by human)
```

**Causa.** Hasta que una persona apruebe las zonas rojas, solo se pueden construir Spikes.

**Solución.** Lee `gdd/zones.toml` y apruébalas en la sentada de tu Change, o con
`bin/gdd gate zones ok --by human`. Consulta [Mapea y aprueba las zonas rojas](red-zones.md).

## Zonas rojas cambiadas después de aprobarse

```text
Red zones: pending · Red zones changed after their approval: a human must approve them again.
```

**Causa.** Se añadió, quitó o editó una zona roja después de su aprobación. Se rechaza todo
Change que no sea un Spike hasta que se vuelvan a aprobar.

**Solución.** Lee qué cambió en el historial de `gdd/zones.toml` y luego ejecuta
`bin/gdd gate zones ok --by human`.

## Escenarios nuevos o cambiados sin G1

```text
New or changed scenarios without G1: a human must approve them (the gdd-change command).
```

**Causa.** El Change creó o modificó un fichero `.feature` fuera de `features/regression/`, y
nadie lo aprobó. Un KO en G1 detiene el lanzamiento de la misma forma.

**Solución.** Celebra la sentada (`bin/gdd sitting CHANGE`) o ejecuta
`bin/gdd gate G1 ok CHANGE --by human`. Consulta [Escribe y aprueba
escenarios](write-scenarios.md).

## Change rojo: una persona debe aprobar el plan

```text
error: Red change: a human must approve the plan (gate 'plan').
```

**Causa.** El Change es rojo: planifica tocar una ruta de una zona roja, o ya la ha tocado. Un
Change rojo espera a que una persona apruebe su plan, que está en
`.gdd/changes/CHANGE/plan.md`. Un Change puede volverse rojo en un relanzamiento, cuando su
Builder ha tocado una ruta roja: `bin/gdd classify CHANGE` tiene en cuenta lo que ha tocado.

**Solución.** Lee el plan y apruébalo en la sentada, o con
`bin/gdd gate plan ok CHANGE --by human --note "..."`.

## Reviewer y Builder son de la misma familia de modelos

```text
error: Reviewer and Builder are the same model family: edit gdd/models.toml.
```

**Causa.** El Reviewer debe ser de una familia de modelos distinta de la del Builder, para no
compartir sus puntos ciegos. `gdd init` se niega del mismo modo cuando todos los agentes de la
máquina son de una sola familia:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

**Solución.** Asigna a `[reviewer]` en `gdd/models.toml` un agente y un modelo de otra familia
(consulta `[families]` en el mismo fichero). Si solo tienes una familia, prepara el proyecto
con `gdd init --accept-same-family`: el Change lo indicará entonces en su Decision Inbox.
Consulta [Elige agentes y modelos](agents-and-models.md).

## No se encuentra el programa de un agente en el PATH

```text
cannot find the claude program of Claude Code on PATH
error: no coding agent found on the PATH (claude, codex, opencode): install one or name it with --agents
```

**Causa.** Antes de ejecutar nada, la receta comprueba que el programa del agente de cada rol
está en el `PATH`; `gdd init` busca agentes en el `PATH` cuando no se le pasa `--agents`.

**Solución.** Instala el agente e inicia sesión con su propio comando, o cambia el rol en
`gdd/models.toml`. Consulta [Instala gdd](install.md).

## El árbol de trabajo tiene cambios sin commit

```text
error: The working tree has uncommitted changes. Commit them before launching a change.
```

**Causa.** `gdd launch` lleva el checkout principal a main y ejecuta el Change en su propio
worktree; rechaza los cambios sin commit en ficheros versionados que estén fuera de la rama del
Change.

**Solución.** Haz commit de ellos o guárdalos con `git stash`, y vuelve a lanzar. En la propia
rama del Change, el trabajo anterior se guarda en un commit por ti antes del relanzamiento.

## Ejecuta gdd launch desde el checkout principal

```text
error: run gdd resume from the main checkout (/home/me/notes): the worktrees of the changes are made and removed from there
```

**Causa.** Has ejecutado `gdd launch` o `gdd resume` dentro del worktree de un Change; el
mensaje nombra el comando.

**Solución.** Haz `cd` al checkout principal y ejecútalo allí.

## Se ha alcanzado el tope de Changes en construcción

```text
2 change(s) building, the cap is 2: cart-total, export-csv
login-rate-limit can launch once one of those building reaches G4 or is parked, or with a higher [parallel] max in gdd/rules.toml.
```

**Causa.** Ya hay `[parallel] max` Changes en construcción. Los Changes aparcados o que esperan
en G4 no cuentan.

**Solución.** Espera, o sube `[parallel] max` en main. Consulta [Ejecuta Changes en
paralelo](parallel-changes.md).

## G1 rechazada: los ejemplos ocultos usan frases de paso desconocidas

```text
error: G1 refused: hidden examples use step phrasings that no visible scenario has, so test mode can never write them. Reuse visible sentences with new data:
  - a notebook with one note "…" and no tags
  - the tags field of row # is empty
```

**Causa.** Un ejemplo oculto usa una frase que no tiene ningún escenario visible. El modo test
nunca ve los ejemplos ocultos, así que nunca podría escribir ese paso.

**Solución.** Reescribe el ejemplo oculto con las frases de los escenarios visibles y datos
nuevos, o añade la frase a un escenario visible. Haz commit en la rama y vuelve a aprobar.

## gdd sitting necesita un terminal

```text
error: gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok export-csv --by human (and gdd gate zones ok --by human, gdd gate plan ok export-csv --by human for a red change)
```

**Causa.** La sentada es una pantalla, y no hay ningún terminal (un script, la CI, el chat de
un agente).

**Solución.** Ejecútala en un terminal, o registra cada parte con los comandos `gdd gate` que
nombra el mensaje.

## Un Builder o un Reviewer no puede abrir ni cerrar puertas

```text
error: A Builder or Reviewer cannot open or close gates.
```

**Causa.** El comando se ejecutó con `GDD_ROLE` puesto a un rol de agente. Los agentes no
pueden registrar aprobaciones, responder a G4 ni hacer una release.

**Solución.** Ejecútalo tú, en un shell sin `GDD_ROLE`.

## Aparcado: G2 sigue en rojo tras 3 reintentos y 1 replanificación

```text
Change CHANGE parked: G2 still red after 3 retries and 1 replan. Message in .gdd/changes/CHANGE/inbox.md
```

**Causa.** Tres rondas del Builder y una replanificación del Orchestrator no consiguieron poner
G2 en verde. Los números salen de `[limits] builder_retries` y `replans` de `gdd/rules.toml`.

**Solución.** Lee `.gdd/changes/CHANGE/g2.json` para ver qué falló, y las entradas de más abajo
para los fallos de Path Guard y de alcance. Consulta [Vuelve a poner en marcha un Change
aparcado](parked-change.md).

## Ruta protegida para builder-build

```text
gdd/rules.toml: protected path for builder-build
```

**Causa.** Path Guard ha encontrado una ruta que el rol no debe tocar desde que empezó el
build: el arnés, para todos; los escenarios y los pasos escritos en modo test, para el modo
build; el código de producción, para el modo test. Las rutas son las de `[protected]` de
`gdd/rules.toml`, tal como estaban cuando empezó el build.

Hay una segunda causa que pilla a quien arregla a mano un Change aparcado: si haces un commit a
mano en la rama de un Change que contiene trabajo de los agentes sin commit, el inicio de su
build se mueve, y entonces Path Guard culpa al modo build de los pasos que escribió el modo
test.

**Solución.** Deshaz la edición que hizo el agente en la ruta protegida. Si hiciste un commit a
mano, haz primero commit del trabajo de los agentes y luego del tuyo, como muestra [Arregla a
mano en el worktree](parked-change.md#4-arreglar-a-mano-en-el-worktree).

## Los comandos de la puerta no se ejecutaron: primero debe pasar Path Guard

```text
{"name":"path-guard","tail":"gdd/rules.toml: protected path for builder-build\nThe gate commands did not run: Path Guard must pass first."}
```

**Causa.** Cada puerta ejecuta Path Guard antes que sus comandos. Cuando falla, no se ejecuta
ningún comando, así que un agente que reescribió `gdd/commands.toml` nunca consigue que se
ejecuten sus comandos.

**Solución.** La misma que para [una ruta protegida](#ruta-protegida-para-builder-build).

## Escenario aprobado en G1 y modificado después

```text
{"ok":false,"failures":["features/export_csv.feature: scenario approved in G1 and modified afterwards"]}
```

**Causa.** Ha cambiado un fichero de escenarios sellado. Path Guard detiene por ello a todos
los roles.

**Solución.** Restaura el fichero aprobado o, si el escenario tiene que cambiar, haz commit de
la nueva versión en la rama y vuelve a aprobar G1. Consulta [Cambia un escenario después de
aprobarlo](write-scenarios.md#8-cambia-un-escenario-después-de-aprobarlo).

## Fuera del paquete y rojo

```text
src/store/index.rs: outside the package and red (the change is yellow); remove it, or the Orchestrator widens the scope
```

**Causa.** En modo build, G2 también comprueba el alcance: una ruta que el Change no planificó
se acepta cuando su color no es más alto que el del Change, y se anota en la Decision Inbox;
una de color más alto falla, porque el Change necesitaría puertas que no se le asignaron.

**Solución.** Borra el fichero en el worktree y relanza, o consérvalo: el relanzamiento
clasifica el Change según lo que ha tocado, así que se vuelve rojo y espera a que se apruebe su
plan. Consulta [Por qué se aparcan los Changes](parked-change.md#por-qué-se-aparcan-los-changes).

## Path Guard ha fallado tras el modo test

```text
Change CHANGE parked: Path Guard failed after test mode: src/x.rs: protected path for builder-test. Message in .gdd/changes/CHANGE/inbox.md
```

**Causa.** El Builder en modo test ha escrito código de producción, que solo puede escribir el
modo build.

**Solución.** Borra esos ficheros del worktree y relanza. Si vuelve a pasar, comprueba que
`builder_test` en `[protected]` coincide con el sitio donde vive tu código de producción.

## Cambio en el estado de GDD

```text
Change CHANGE parked: the Builder in build mode changed GDD state: .gdd/changes/CHANGE/change.json.
```

**Causa.** Antes y después de cada llamada de agente, gdd comprueba que el agente no ha tocado
el estado de su Change en `.gdd/` ni el estado que comparten todos los Changes. Si lo ha
tocado, gdd restaura el estado y aparca el Change. La misma comprobación aparca un Change
cuando el Reviewer edita un fichero (`the Reviewer changed ...: Reviewers only read`).

**Solución.** Lee la salida del agente en `.gdd/changes/CHANGE/outputs/`; comprueba que sus
instrucciones son las que escribió gdd (`bin/gdd agents check`) y luego relanza.

## No respondió en N segundos

```text
Change CHANGE parked: the Builder in build mode did not answer within 3600 seconds.
```

**Causa.** Un agente ha superado su límite de tiempo y gdd lo ha detenido, junto con todos los
procesos que había lanzado. El límite es `[limits] agent_timeout_s` de `gdd/rules.toml`: 3600
segundos si el fichero no lo fija. gdd también aparca un Change cuyo agente termina con un
error (`failed (exit N)`).

**Solución.** Ejecuta el agente a mano una vez para comprobar su inicio de sesión y su cuota.
Si el trabajo simplemente es lento, sube `agent_timeout_s` en main, y luego relanza.

## El Spike ha agotado su presupuesto

```text
parked: the spike spent 5.00 USD of its budget of 5.00 USD
```

**Causa.** Las llamadas de agente de un Spike, o de una de sus variantes, han costado
`[spike] max_usd`.

**Solución.** Lee lo que ha encontrado hasta ahora y luego sube el presupuesto en main y
relanza, o responde con lo que ya sabes. Consulta [Explora con un Spike](spikes.md).

## Tiempo agotado: sube limits.command_seconds

```text
timed out (1800s): raise limits.command_seconds in gdd/rules.toml
```

**Causa.** Un comando de puerta o de release ha tardado más que `[limits] command_seconds`
(1800 segundos por defecto) y gdd lo ha detenido. La puerta falla en ese comando.

**Solución.** En una máquina lenta, o con dos Changes en G3 a la vez, sube `command_seconds` en
`gdd/rules.toml`, en main. Tiene que ser un número entero de segundos mayor que 0; si no, todas
las puertas se detienen con `limits.command_seconds must be a whole number of seconds above 0`.

## El mutation testing agota el tiempo

```text
2 mutant(s) hung again: counted as caught
```

**Causa.** En la ficha de Rust, un mutante puede agotar el tiempo porque se cuelga o porque la
máquina está ocupada. Cuando solo quedan mutantes con el tiempo agotado, el comando los vuelve
a ejecutar, de uno en uno y con al menos 120 segundos por test; uno que se vuelve a colgar
cuenta como cazado, y uno que ahora sobrevive hace fallar la puerta. Los mutantes que no se
cazaron no tienen segunda oportunidad.

**Solución.** Nada, si la puerta pasa. Cuando lo que agota el tiempo es el comando de mutación
entero, sube `command_seconds`, o evita tener dos Changes en G3 a la vez (consulta [Ejecuta
Changes en paralelo](parallel-changes.md)). Para los comandos en sí, consulta [Ajusta las
puertas](gates-and-commands.md).

## gdd/commands.toml no tiene comandos para esta puerta

```text
gdd/commands.toml has no commands for this gate. The Setup fills them in.
```

**Causa.** Todos los comandos de la puerta están vacíos. Una puerta sin comandos falla
(`G2 not configured`): una puerta vacía dejaría pasar cualquier cosa.

**Solución.** Rellena los comandos, en main. Consulta [Ajusta las
puertas](gates-and-commands.md).

## Cero escenarios cuenta como fallo

```text
Zero scenarios counts as a failure.
```

**Causa.** G3 cuenta los escenarios de `features/`, y `[acceptance] required = true` en
`gdd/rules.toml` hace que cero sea un fallo.

**Solución.** Escribe el primer escenario (el Setup escribe un escenario de humo). Consulta
[Escribe y aprueba escenarios](write-scenarios.md).

## Main ha avanzado entretanto y la fusión tiene conflictos

```text
main moved under it and the merge has conflicts in src/export.rs: merge main into change/export-csv in its worktree (/home/me/notes/../notes.worktrees/export-csv), settle the conflicts, commit, then gdd launch export-csv
```

**Causa.** Otro Change se ha fusionado antes, y git no puede fusionar el último main en este.

**Solución.** Sigue lo que dice el mensaje: consulta [Resuelve los conflictos de fusión con
main](parked-change.md#6-resolver-los-conflictos-de-fusión-con-main).

## G3 en rojo sobre el último main

```text
Change CHANGE parked: G3 red on top of the latest main. Message in .gdd/changes/CHANGE/inbox.md
```

**Causa.** Main se ha fusionado sin conflictos en el Change, pero G3 falla con lo que ha traído
main.

**Solución.** Lee `g3.json`, corrige el fallo en la rama, dentro del worktree, haz commit y
relanza.

## KO del Reviewer o G3 en rojo tras una corrección

```text
Change CHANGE parked: Reviewer KO or G3 red after one fix. Message in .gdd/changes/CHANGE/inbox.md
```

**Causa.** El Reviewer ha dicho KO, el Builder ha tenido su única corrección y el Reviewer ha
vuelto a decir KO (o G3 se ha roto durante la corrección).

**Solución.** Lee el último `outputs/reviewer-<n>.md`. Cuando el hallazgo solo se puede
arreglar en los pasos, el modo build no puede hacerlo (los tiene protegidos): arréglalos a
mano. Consulta [Vuelve a poner en marcha un Change aparcado](parked-change.md).

## La fusión necesita el checkout principal sin cambios pendientes

```text
Change CHANGE parked: the merge needs the main checkout (/home/me/notes) without uncommitted changes in tracked files: commit or stash them.
```

**Causa.** Alguien ha editado un fichero versionado en el checkout principal mientras se
ejecutaba el Change, y la fusión se hace allí.

**Solución.** Haz commit de la edición o guárdala con `git stash`, y luego ejecuta
`bin/gdd launch CHANGE`.

## No está en pausa en G4

```text
error: Change export-csv is not paused at G4.
```

**Causa.** `gdd resume` solo responde a un Change que espera en G4. Tras una observación
fallida, el Change se ha revertido y ya no espera.

**Solución.** Comprueba el estado con `bin/gdd status`. Un Change revertido necesita un Hotfix.
Consulta [Responde a la puerta de salida](release-gate.md).

## Elige una variante

```text
error: pick a variant: gdd resume notes-storage --g4 ok --variant 1|2
```

**Causa.** Para responder OK o polish a un Spike con variantes hace falta el número de la
variante.

**Solución.** Añade `--variant <n>` con una variante que haya pasado su puerta. Consulta
[Explora con un Spike](spikes.md#6-comparar-las-variantes-y-elegir-una).

## Siguientes pasos

- [Vuelve a poner en marcha un Change aparcado](parked-change.md): el camino completo de vuelta
  para un Change aparcado.
- [Referencia de comandos](../reference/cli.md): todos los comandos y opciones.
- [Ficheros que escribe gdd](../reference/files.md): dónde están los informes, las salidas y
  los expedientes.
- [Mantener honestos a los agentes](../explanation/safety.md): por qué Path Guard y las
  comprobaciones del estado son tan estrictos.
