# Solución de problemas

Busca el mensaje que imprimió gdd, o la forma en que se detuvo un Change, y haz lo que dice su
entrada. Cada entrada muestra lo que ves, por qué pasa en una línea y qué hacer.

Los mensajes se citan tal como los imprime gdd, en inglés. `CHANGE` representa el id de tu
Change, y los números, rutas e ids de los mensajes son ejemplos. Si un Change se ha aparcado,
empieza por [Volver a poner en marcha un Change aparcado](parked-change.md).

## Antes de empezar

- Estado: ejecuta primero [`gdd status`](../reference/cli/status.md). Te dice cómo están G0, las
  zonas rojas, el autotest y cada Change.
- Bandeja: para un Change concreto, [`gdd inbox CHANGE`](../reference/cli/inbox.md) muestra su
  Decision Inbox. `.gdd/changes/CHANGE/` guarda los informes de las puertas (`g2.json`,
  `g3.json`) y lo que dijo cada agente (`outputs/`).

## Encuentra tu mensaje

| Cuándo | Entradas |
|---|---|
| Al preparar el proyecto | [Falló la primera instalación](#falló-la-primera-instalación) |
| Antes de que empiece el build | [G0 pendiente](#g0-pendiente-ejecuta-el-setup) · [G0 no se puede abrir](#g0-no-se-puede-abrir) · [Autotest: G2 en rojo](#autotest-g2-está-en-rojo-sin-el-test-que-falla) · [Zonas rojas sin aprobar](#zonas-rojas-aún-sin-aprobar) · [Zonas rojas cambiadas](#zonas-rojas-cambiadas-después-de-aprobarse) · [Escenarios sin G1](#escenarios-nuevos-o-cambiados-sin-g1) · [Plan de un Change rojo](#change-rojo-una-persona-debe-aprobar-el-plan) · [Misma familia de modelos](#reviewer-y-builder-son-de-la-misma-familia-de-modelos) · [Agente no encontrado](#no-se-encuentra-el-programa-de-un-agente-en-el-path) · [Cambios sin commit](#el-árbol-de-trabajo-tiene-cambios-sin-commit) · [No es el checkout principal](#ejecuta-gdd-launch-desde-el-checkout-principal) · [El tope](#se-ha-alcanzado-el-tope-de-changes-en-construcción) |
| En una sentada | [G1 rechazada](#g1-rechazada-los-ejemplos-ocultos-usan-frases-de-paso-desconocidas) · [Sin terminal](#gdd-sitting-necesita-un-terminal) · [El Builder no puede registrar](#un-builder-o-un-reviewer-no-puede-abrir-ni-cerrar-puertas) · [Un agente no puede ejecutar un comando](#los-builders-y-los-reviewers-no-pueden-ejecutar-gdd-command) |
| Durante el build | [G2 sigue en rojo](#aparcado-g2-sigue-en-rojo-tras-3-reintentos-y-1-replanificación) · [Ruta protegida](#ruta-protegida-para-builder-build) · [Los comandos no se ejecutaron](#los-comandos-de-la-puerta-no-se-ejecutaron-primero-debe-pasar-path-guard) · [Escenario aprobado modificado](#escenario-aprobado-en-g1-y-modificado-después) · [Fuera del paquete](#fuera-del-paquete-y-rojo) · [El modo test escribió código](#path-guard-ha-fallado-tras-el-modo-test) · [El agente cambió el estado de GDD](#cambio-en-el-estado-de-gdd) · [El agente no respondió](#no-respondió-en-n-segundos) · [Presupuesto del Spike](#el-spike-ha-agotado-su-presupuesto) |
| En G2 y G3 | [Comando que agota el tiempo](#tiempo-agotado-sube-limitscommand_seconds) · [Timeouts del mutation testing](#el-mutation-testing-agota-el-tiempo) · [Sin configurar](#gddcommandstoml-no-tiene-comandos-para-esta-puerta) · [Cero escenarios](#cero-escenarios-cuenta-como-fallo) |
| En la fusión | [Conflictos con main](#main-ha-avanzado-entretanto-y-la-fusión-tiene-conflictos) · [G3 en rojo sobre main](#g3-en-rojo-sobre-el-último-main) · [KO del Reviewer](#ko-del-reviewer-o-g3-en-rojo-tras-una-corrección) · [Checkout principal con cambios](#la-fusión-necesita-el-checkout-principal-sin-cambios-pendientes) |
| En G4 | [No está en pausa en G4](#no-está-en-pausa-en-g4) · [Elige una variante](#elige-una-variante) |

## Falló la primera instalación

<!-- Real output of the gdd init form, gdd 0.1.0, with an install made to fail on purpose; the
     line the failing tool printed is left out. -->

```text
python: uv lock && uv sync --locked
...
error: python: uv lock && uv sync --locked failed (exit 2)
The harness is set up and committed. Fix the install, then run gdd bootstrap.
```

**Por qué.** El formulario de Setup escribió el arnés y lo guardó en un commit como
`gdd init 0.1.0`, y luego falló la primera instalación de un stack: no hay red, falta una
herramienta como `uv`, `npm` o `cargo`, o una dependencia no se resuelve.

**Qué hacer.** Lee la línea que hay encima del error: es lo que dijo la herramienta. Arregla
eso y luego ejecuta [`gdd bootstrap`](../reference/cli/bootstrap.md), que vuelve a ejecutar la
instalación y hace commit de los ficheros de bloqueo (*lock files*). No vuelvas a ejecutar
`gdd init`, porque el proyecto ya está preparado. Después arranca tu agente y ejecuta el
comando `gdd-setup`.

## G0 pendiente: ejecuta el Setup

```text
error: G0 pending: run the Setup (the gdd-setup command).
```

**Por qué.** Ningún Change empieza antes de que la puerta de preparación (G0, *Setup Gate*) esté
abierta, y `gdd status` muestra `G0: pending`.

**Qué hacer.** Arranca tu agente y ejecuta el comando `gdd-setup` (`/gdd-setup` en Claude Code
y OpenCode, `$gdd-setup` en Codex). Escribe el Brief, ejecuta
[`gdd selftest`](../reference/cli/selftest.md) y abre G0 con
[`gdd gate G0 ok --by machines`](../reference/cli/gate.md).

## G0 no se puede abrir

```text
error: G0 cannot open: the harness self-test is not 4/4 (gdd selftest).
error: G0 cannot open: docs/brief.md (Project Brief) is missing.
error: G0 is opened by the machines (--by machines): Brief written and harness 4/4.
```

**Por qué.** Solo las máquinas abren G0, y solo cuando existe el Project Brief y el último
autotest pasó las cuatro comprobaciones.

**Qué hacer.** Escribe `docs/brief.md`, ejecuta `gdd selftest` hasta que pase y luego ejecuta
`gdd gate G0 ok --by machines`. El resultado del autotest está en `.gdd/selftest.json`.

## Autotest: G2 está en rojo sin el test que falla

```text
KO a failing test turns G2 red (G2 is red without it: lint)
KO a failing test turns G2 red (gdd/debt.toml: invalid TOML: TOML parse error at line 17, column 1
```

**Por qué.** El autotest necesita G2 en verde antes de plantar su test que falla, y el comando
nombrado falla en main (por encima de su deuda conocida, si la tiene), o `gdd/debt.toml` no se
puede leer.

**Qué hacer.**

- Comando nombrado: haz que pase en main. En un proyecto que ya tiene código, registra lo que
  falla hoy con [`gdd adopt`](../reference/cli/adopt.md) (consulta [Llevar gdd a un proyecto
  existente](adopt-existing-project.md#3-deuda-conocida-lo-que-hoy-falla)). Si el comando da
  más problemas de los que marca su recuento, corrige sus problemas nuevos.
- `gdd/debt.toml` nombrado: corrige el fichero en la línea que da el mensaje.

Después vuelve a ejecutar `gdd selftest`.

## Zonas rojas aún sin aprobar

```text
error: Red zones not approved yet: the Orchestrator proposes them at the end of Discover. (gdd gate zones ok --by human)
```

**Por qué.** Hasta que una persona apruebe las zonas rojas, solo se pueden construir Spikes.

**Qué hacer.** Lee `gdd/zones.toml` y luego aprueba las zonas en la sentada de tu Change, o con
`gdd gate zones ok --by human`. Consulta [Mapear y aprobar zonas rojas](red-zones.md).

## Zonas rojas cambiadas después de aprobarse

```text
Red zones: pending · Red zones changed after their approval: a human must approve them again.
```

**Por qué.** Alguien añadió, quitó o editó una zona roja después de su aprobación, así que gdd
rechaza todo Change que no sea un Spike hasta que una persona vuelva a aprobar las zonas.

**Qué hacer.** Lee qué cambió en el historial de `gdd/zones.toml` y luego ejecuta
`gdd gate zones ok --by human`.

## Escenarios nuevos o cambiados sin G1

```text
New or changed scenarios without G1: a human must approve them (the gdd-change command).
```

**Por qué.** El Change creó o modificó un fichero `.feature` fuera de `features/regression/` y
nadie lo aprobó, o G1 recibió un KO.

**Qué hacer.** Celebra la sentada ([`gdd sitting CHANGE`](../reference/cli/sitting.md)) o
ejecuta `gdd gate G1 ok CHANGE --by human`. Consulta [Aprobar los
escenarios](write-scenarios.md).

## Change rojo: una persona debe aprobar el plan

```text
error: Red change: a human must approve the plan (gate 'plan').
```

**Por qué.** El Change planifica tocar una ruta de una zona roja, o ya la ha tocado, así que
espera a que una persona apruebe su plan en `.gdd/changes/CHANGE/plan.md`.

**Qué hacer.** Lee el plan y apruébalo en la sentada, o con
`gdd gate plan ok CHANGE --by human --note "..."`. Si el Change se volvió rojo en un
relanzamiento, su Builder tocó una ruta roja: [`gdd classify CHANGE`](../reference/cli/classify.md)
muestra lo que tiene en cuenta.

## Reviewer y Builder son de la misma familia de modelos

```text
error: Reviewer and Builder are the same model family: edit gdd/models.toml.
```

**Por qué.** El Reviewer tiene que ser de una familia de modelos distinta de la del Builder,
para no compartir sus puntos ciegos.

[`gdd init`](../reference/cli/init.md) se niega del mismo modo cuando todos los agentes de la
máquina son de una sola familia:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

**Qué hacer.** Asigna a `[reviewer]` en `gdd/models.toml` un agente y un modelo de otra familia
(consulta `[families]` en el mismo fichero). Si solo tienes una familia, prepara el proyecto
con `gdd init --accept-same-family`, y cada Change lo indicará entonces en su Decision Inbox.
Consulta [Elegir agentes y modelos](agents-and-models.md).

## No se encuentra el programa de un agente en el PATH

```text
cannot find the claude program of Claude Code on PATH
error: no coding agent found on the PATH (claude, codex, opencode): install one or name it with --agents
```

**Por qué.** El programa del agente de un rol no está en el `PATH`, que la receta comprueba
antes de ejecutar nada y en el que `gdd init` busca cuando no se le pasa `--agents`.

**Qué hacer.** Instala el agente e inicia sesión con su propio comando, o cambia el rol en
`gdd/models.toml`. Consulta [Instalar gdd](install.md).

## El árbol de trabajo tiene cambios sin commit

```text
error: The working tree has uncommitted changes. Commit them before launching a change.
```

**Por qué.** [`gdd launch`](../reference/cli/launch.md) lleva el checkout principal a main, así
que rechaza los cambios sin commit en ficheros versionados que estén fuera de la rama del
Change.

**Qué hacer.** Haz commit de ellos o guárdalos con `git stash`, y vuelve a lanzar. En la propia
rama del Change, gdd guarda por ti el trabajo anterior en un commit antes del relanzamiento.

## Ejecuta gdd launch desde el checkout principal

```text
error: run gdd resume from the main checkout (/home/me/notes): the worktrees of the changes are made and removed from there
```

**Por qué.** Has ejecutado `gdd launch` o [`gdd resume`](../reference/cli/resume.md) dentro del
worktree de un Change; el mensaje nombra el comando.

**Qué hacer.** Ve al checkout principal (haz `cd` a la carpeta que nombra el mensaje) y ejecuta
allí el comando.

## Se ha alcanzado el tope de Changes en construcción

```text
2 change(s) building, the cap is 2: cart-total, export-csv
login-rate-limit can launch once one of those building reaches G4 or is parked, or with a higher [parallel] max in gdd/rules.toml.
```

**Por qué.** Ya hay `[parallel] max` Changes en construcción; los Changes aparcados o que
esperan en G4 no cuentan.

**Qué hacer.** Espera, o sube `[parallel] max` en main. Consulta [Ejecutar Changes en
paralelo](parallel-changes.md).

## G1 rechazada: los ejemplos ocultos usan frases de paso desconocidas

```text
error: G1 refused: hidden examples use step phrasings that no visible scenario has, so test mode can never write them. Reuse visible sentences with new data:
  - a notebook with one note "…" and no tags
  - the tags field of row # is empty
```

**Por qué.** Un ejemplo oculto usa una frase que no tiene ningún escenario visible, y el modo
test, que nunca ve los ejemplos ocultos, nunca podría escribir ese paso.

**Qué hacer.** Reescribe el ejemplo oculto con las frases de los escenarios visibles y datos
nuevos, o añade la frase a un escenario visible. Haz commit en la rama y vuelve a aprobar.

## gdd sitting necesita un terminal

```text
error: gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok export-csv --by human (and gdd gate zones ok --by human, gdd gate plan ok export-csv --by human for a red change)
```

**Por qué.** La sentada es una pantalla, y no hay ningún terminal donde mostrarla (un script, la
CI, el chat de un agente).

**Qué hacer.** Ejecútala en un terminal, o registra cada parte con los comandos `gdd gate` que
nombra el mensaje.

## Un Builder o un Reviewer no puede abrir ni cerrar puertas

```text
error: A Builder or Reviewer cannot open or close gates.
```

**Por qué.** El comando se ejecutó con `GDD_ROLE` puesto a un rol de agente, y los agentes no
pueden registrar aprobaciones, responder a G4 ni publicar.

**Qué hacer.** Ejecútalo tú, en un shell sin `GDD_ROLE`.

## Los Builders y los Reviewers no pueden ejecutar gdd COMMAND

```text
error: Builders and Reviewers cannot run gdd selftest: an agent gdd started may only run gdd g2, gdd guard, gdd note, gdd status, gdd classify, gdd inbox, gdd report and gdd scope.
```

**Por qué.** gdd pone `GDD_ROLE` a cada agente que arranca, y un agente así solo puede
ejecutar los comandos que enumera el mensaje
([Roles](../reference/roles.md#qué-puede-ejecutar-un-agente) explica por qué).

**Qué hacer.**

- Agente: si le ha pasado a un agente, no hay nada que arreglar. El agente intentó algo fuera
  de su trabajo, y no pasó nada.
- Tu propio shell: si lo ves ahí, `GDD_ROLE` está puesto en tu shell por error. Ejecuta
  `unset GDD_ROLE` y vuelve a intentarlo.

## Aparcado: G2 sigue en rojo tras 3 reintentos y 1 replanificación

```text
Change CHANGE parked: G2 still red after 3 retries and 1 replan. Message in .gdd/changes/CHANGE/inbox.md
```

**Por qué.** Tres rondas del Builder y una replanificación del Orchestrator no consiguieron
poner G2 en verde; los números salen de `[limits] builder_retries` y `replans` de
`gdd/rules.toml`.

**Qué hacer.** Lee `.gdd/changes/CHANGE/g2.json` para ver qué falló. Para los fallos de Path
Guard y de alcance, consulta las entradas de más abajo. Consulta [Volver a poner en marcha un
Change aparcado](parked-change.md).

## Ruta protegida para builder-build

```text
gdd/rules.toml: protected path for builder-build
```

**Por qué.** Path Guard ha encontrado una ruta que el rol no debe tocar: el arnés para todos,
los escenarios y los pasos para el modo build, el código de producción para el modo test
(`[protected]` de `gdd/rules.toml`, tal como estaba cuando empezó el build).

**Qué hacer.**

1. Deshaz la edición que hizo el agente en la ruta protegida.
2. Si hiciste un commit a mano en la rama del Change mientras contenía trabajo de los agentes
   sin commit, Path Guard culpa ahora al modo build de los pasos que escribió el modo test. Haz
   primero commit del trabajo de los agentes y luego del tuyo, como muestra [Arreglar a mano
   en el worktree](parked-change.md#4-arregla-a-mano-en-el-worktree).

## Los comandos de la puerta no se ejecutaron: primero debe pasar Path Guard

```text
{"name":"path-guard","tail":"gdd/rules.toml: protected path for builder-build\nThe gate commands did not run: Path Guard must pass first."}
```

**Por qué.** Cada puerta ejecuta Path Guard antes que sus comandos, y cuando falla no se ejecuta
ninguno, así que un agente que reescribió `gdd/commands.toml` nunca consigue que se ejecuten sus
comandos.

**Qué hacer.** Haz lo mismo que para [una ruta protegida](#ruta-protegida-para-builder-build).

## Escenario aprobado en G1 y modificado después

```text
{"ok":false,"failures":["features/export_csv.feature: scenario approved in G1 and modified afterwards"]}
```

**Por qué.** Ha cambiado un fichero de escenarios sellado, y Path Guard detiene por ello a todos
los roles.

**Qué hacer.** Restaura el fichero aprobado. Si el escenario tiene que cambiar, haz commit de la
nueva versión en la rama y vuelve a aprobar G1. Consulta [Cambiar un escenario después de
aprobarlo](write-scenarios.md#5-cambia-un-escenario-después-de-aprobarlo).

## Fuera del paquete y rojo

```text
src/store/index.rs: outside the package and red (the change is yellow); remove it, or the Orchestrator widens the scope
```

**Por qué.** En modo build, G2 hace fallar una ruta que el Change no planificó cuando su color es
más alto que el del Change, porque el Change necesitaría puertas que no se le asignaron.

**Qué hacer.** Elige una opción:

- Borrar: borra el fichero en el worktree y relanza.
- Conservar: relanza tal cual. El relanzamiento clasifica el Change según lo que ha tocado, así
  que se vuelve rojo y espera a que se apruebe su plan.

Consulta [Por qué se aparcan los Changes](parked-change.md#por-qué-se-aparcan-los-changes).

## Path Guard ha fallado tras el modo test

```text
Change CHANGE parked: Path Guard failed after test mode: src/x.rs: protected path for builder-test. Message in .gdd/changes/CHANGE/inbox.md
```

**Por qué.** El Builder en modo test ha escrito código de producción, que solo puede escribir el
modo build.

**Qué hacer.** Borra esos ficheros del worktree y relanza. Si vuelve a pasar, comprueba que
`builder_test` en `[protected]` coincide con el sitio donde vive tu código de producción.

## Cambio en el estado de GDD

```text
Change CHANGE parked: the Builder in build mode changed GDD state: .gdd/changes/CHANGE/change.json.
```

**Por qué.** Un agente ha tocado el estado que gdd guarda en `.gdd/`, así que gdd ha restaurado
el estado y ha aparcado el Change.

**Qué hacer.** Lee la salida del agente en `.gdd/changes/CHANGE/outputs/`. Comprueba que sus
instrucciones son las que escribió gdd ([`gdd agents check`](../reference/cli/agents.md)) y
luego relanza. Un Reviewer que edita un fichero aparca el Change del mismo modo
(`the Reviewer changed ...: Reviewers only read`).

## No respondió en N segundos

```text
Change CHANGE parked: the Builder in build mode did not answer within 3600 seconds.
```

**Por qué.** Un agente ha superado `[limits] agent_timeout_s` de `gdd/rules.toml` (3600 segundos
si el fichero no lo fija), y gdd lo ha detenido junto con todos los procesos que había lanzado.

**Qué hacer.** Ejecuta el agente a mano una vez para comprobar su inicio de sesión y su cuota.
Si el trabajo solo es lento, sube `agent_timeout_s` en main y luego relanza. Un Change cuyo
agente termina con un error se aparca del mismo modo, con `failed (exit N)`.

## El Spike ha agotado su presupuesto

```text
parked: the spike spent 5.00 USD of its budget of 5.00 USD
```

**Por qué.** Las llamadas de agente de un Spike, o de una de sus variantes, han costado
`[spike] max_usd`.

**Qué hacer.** Lee lo que ha encontrado hasta ahora. Después, o sube el presupuesto en main y
relanza, o responde con lo que ya sabes. Consulta [Explorar con un Spike](spikes.md).

## Tiempo agotado: sube limits.command_seconds

```text
timed out (1800s): raise limits.command_seconds in gdd/rules.toml
```

**Por qué.** Un comando de puerta o de publicación ha tardado más que `[limits] command_seconds`
(1800 segundos por defecto), así que gdd lo ha detenido y la puerta falla en ese comando.

**Qué hacer.** En una máquina lenta, o con dos Changes en G3 a la vez, sube `command_seconds` en
`gdd/rules.toml`, en main. Usa un número entero de segundos mayor que 0; si no, todas las
puertas se detienen con `limits.command_seconds must be a whole number of seconds above 0`.

## El mutation testing agota el tiempo

```text
2 mutant(s) hung again: counted as caught
```

**Por qué.** En la ficha de Rust, los mutantes que agotaron el tiempo se vuelven a ejecutar de
uno en uno, y uno que se vuelve a colgar cuenta como detectado, mientras que uno que ahora
sobrevive hace fallar la puerta.

**Qué hacer.** Si la puerta pasa, nada. Si lo que agota el tiempo es el comando de mutación
entero, sube `command_seconds`, o evita tener dos Changes en G3 a la vez (consulta [Ejecutar
Changes en paralelo](parallel-changes.md)). Para los comandos en sí, consulta [Ajustar las
puertas](gates-and-commands.md).

## gdd/commands.toml no tiene comandos para esta puerta

```text
gdd/commands.toml has no commands for this gate. The Setup fills them in.
```

**Por qué.** Todos los comandos de la puerta están vacíos, y una puerta sin comandos falla
(`G2 not configured`) porque dejaría pasar cualquier cosa.

**Qué hacer.** Rellena los comandos, en main. Consulta [Ajustar las
puertas](gates-and-commands.md).

## Cero escenarios cuenta como fallo

```text
Zero scenarios counts as a failure.
```

**Por qué.** G3 cuenta los escenarios de `features/`, y `[acceptance] required = true` en
`gdd/rules.toml` hace que cero sea un fallo.

**Qué hacer.** Escribe el primer escenario (el Setup escribe un escenario de humo). Consulta
[Aprobar los escenarios](write-scenarios.md).

## Main ha avanzado entretanto y la fusión tiene conflictos

```text
main moved under it and the merge has conflicts in src/export.rs: merge main into change/export-csv in its worktree (/home/me/notes/../notes.worktrees/export-csv), settle the conflicts, commit, then gdd launch export-csv
```

**Por qué.** Otro Change se ha fusionado antes, y git no puede fusionar el último main en este.

**Qué hacer.** Sigue el mensaje paso a paso: consulta [Resolver los conflictos de fusión con
main](parked-change.md#6-resuelve-los-conflictos-de-fusión-con-main).

## G3 en rojo sobre el último main

```text
Change CHANGE parked: G3 red on top of the latest main. Message in .gdd/changes/CHANGE/inbox.md
```

**Por qué.** Main se ha fusionado sin conflictos en el Change, pero G3 falla con lo que ha
traído main.

**Qué hacer.** Lee `g3.json`, corrige el fallo en la rama, dentro del worktree, haz commit y
relanza.

## KO del Reviewer o G3 en rojo tras una corrección

```text
Change CHANGE parked: Reviewer KO or G3 red after one fix. Message in .gdd/changes/CHANGE/inbox.md
```

**Por qué.** El Reviewer dijo KO, el Builder tuvo su única corrección y el Reviewer volvió a
decir KO, o G3 se rompió durante la corrección.

**Qué hacer.** Lee el último `outputs/reviewer-<n>.md`. Si el hallazgo solo se puede arreglar en
los pasos, arréglalos a mano: el modo build no puede, porque los tiene protegidos. Consulta
[Volver a poner en marcha un Change aparcado](parked-change.md).

## La fusión necesita el checkout principal sin cambios pendientes

```text
Change CHANGE parked: the merge needs the main checkout (/home/me/notes) without uncommitted changes in tracked files: commit or stash them.
```

**Por qué.** Alguien editó un fichero versionado en el checkout principal mientras se ejecutaba
el Change, y la fusión se hace allí.

**Qué hacer.** Haz commit de la edición o guárdala con `git stash`, y luego ejecuta
`gdd launch CHANGE`.

## No está en pausa en G4

```text
error: Change export-csv is not paused at G4.
```

**Por qué.** `gdd resume` solo responde a un Change que espera en G4, y un Change revertido tras
una observación fallida ya no espera.

**Qué hacer.** Comprueba el estado con `gdd status`. Si el Change se revirtió, necesita un
Hotfix. Consulta [Revisar un Change en G4](release-gate.md).

## Elige una variante

```text
error: pick a variant: gdd resume notes-storage --g4 ok --variant 1|2
```

**Por qué.** Un OK o un polish a un Spike con variantes necesita el número de la variante.

**Qué hacer.** Añade `--variant <n>` con una variante que haya pasado su puerta. Consulta
[Explorar con un Spike](spikes.md#6-compara-las-variantes-y-elige-una).

## Siguientes pasos

- [Volver a poner en marcha un Change aparcado](parked-change.md): el camino completo de vuelta
  para un Change aparcado.
- [Referencia de comandos](../reference/cli/index.md): todos los comandos y opciones.
- [Ficheros que escribe gdd](../reference/files.md): dónde están los informes, las salidas y
  los expedientes.
- [Mantener honestos a los agentes](../explanation/safety.md): por qué Path Guard y las
  comprobaciones del estado son tan estrictos.
