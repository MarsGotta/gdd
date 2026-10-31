# Ficheros que escribe gdd

Todos los ficheros y carpetas que gdd escribe en un proyecto, qué guarda cada uno, cuáles se
versionan y qué lleva el commit de fusión de un Change.

## Un proyecto después de gdd init

`gdd init --name notes --platform cli --stack python --agents claude codex` en un repositorio
vacío hace commit de estos ficheros (los comandos de los agentes aparecen una sola vez por
carpeta de agente):

```text
.agents/skills/gdd-<command>/SKILL.md   # Codex: the GDD commands and gdd-stack-python
.claude/skills/gdd-<command>/SKILL.md   # Claude Code: the same
.github/workflows/gdd.yml               # CI: the stack setup, gitleaks, gdd, gdd ci
.gitignore                              # the stack's lines and .gdd/
AGENTS.md                               # instructions for agents, when the project had none
CLAUDE.md                               # @AGENTS.md, when the project had none
docs/brief.md                           # the Project Brief
features/.gitkeep                       # the scenario folder
gdd/commands.toml
gdd/generated.toml
gdd/models.toml
gdd/project.toml
gdd/rules.toml
gdd/zones.toml
osv-scanner-licenses.toml               # from here on: the Python skeleton
osv-scanner.toml
pyproject.toml
src/notes/__init__.py
src/notes/greeting.py
tests/__init__.py
tests/acceptance/__init__.py
tests/acceptance/steps/__init__.py
tests/acceptance/test_features.py
tests/unit/__init__.py
tests/unit/test_greeting.py
```

| Ruta | Qué es |
|---|---|
| `gdd/` | La configuración: consulta [Configuración](configuration.md). |
| `docs/brief.md` | El Project Brief, a partir de `--name`, `--description`, `--platform`, `--stack` y `--sensitive`. El Orchestrator lo completa en Setup. |
| `AGENTS.md`, `CLAUDE.md` | Solo se escriben cuando el proyecto no tiene ninguno; si no, gdd imprime la línea que hay que añadir. |
| `.gitignore` | gdd añade al final las rutas ignoradas del stack y `.gdd/`. |
| Carpetas de agentes | `.claude/skills/<name>/SKILL.md` (Claude Code), `.agents/skills/<name>/SKILL.md` (Codex), `.opencode/commands/<name>.md` (OpenCode), para los agentes de `--agents`. `gdd agents install` las vuelve a escribir; `gdd agents check` nombra cualquier fichero que falte o que se haya editado. |
| Esqueleto | Por stack, solo cuando la carpeta no tiene un proyecto de ese stack: consulta [Stacks](stacks.md). |

`gdd init` nunca sobrescribe un fichero, nunca escribe a través de un enlace y solo hace commit
de lo que escribió (`gdd init <version>`), en un repositorio nuevo si la carpeta no está en
ninguno. gdd no escribe ninguna carpeta `bin/`: las personas, los agentes y la CI ejecutan el
`gdd` del `PATH`. La carpeta [`.gdd/bin/`](#gdd) que aparece más abajo es otra cosa: una
carpeta local para programas que un proyecto deja fuera de git, que gdd protege pero nunca
escribe.

Después, el formulario de Setup de `gdd init` hace la primera instalación de cada stack y hace
commit de los lockfiles, como hace `gdd bootstrap` después de `gdd init` con flags. `gdd adopt`
hace commit de `gdd/debt.toml`.

## Versionado y local

| Ruta | Versionado | Lo escribe |
|---|---|---|
| `gdd/*.toml` | sí | `gdd init`, `gdd adopt`, `gdd upgrade`, una persona; consulta [Quién puede cambiarlos](configuration.md#quién-puede-cambiarlos) |
| `.github/workflows/gdd.yml`, carpetas de agentes, `AGENTS.md`, `CLAUDE.md` | sí | `gdd init`, `gdd agents install` |
| `docs/`, `features/`, código y tests | sí | el Orchestrator y el Builder, a través de Changes |
| `CHANGELOG.md` | sí, cuando el proyecto lo mantiene | `gdd changelog --write`, `gdd release` |
| `.gdd/` | no: `gdd init` añade `.gdd/` a `.gitignore` | gdd, y unos pocos ficheros que escribe el Orchestrator |

El registro de cada Change solo llega a git a través de su
[commit de fusión](#el-commit-de-fusión).

## .gdd/

El estado local: lo que gdd sabe del proyecto y de cada Change. Todos los worktrees de un
repositorio comparten la `.gdd/` de su checkout principal (la raíz del estado), así que un
Change se ejecuta en su propio worktree y aun así lee y escribe los mismos registros. gdd se
niega a ejecutarse en un worktree de un repositorio bare.

```text
.gdd/
├── log.jsonl                 # one JSON line per agent call, gate decision and machine step
├── g0.json                   # G0, opened by the machines
├── zones.json                # the human's approval of the red zones
├── selftest.json             # the last harness self-test
├── headless_sessions.txt     # sessions gdd started for its agents
├── imported_requests.json    # requests of interactive sessions already in the log
├── agents.lock  launch.lock  merge.lock  state.lock
├── adoption/<command>.txt    # gdd adopt: what each failing command said
├── learn/<tool>.txt          # gdd learn: what each Learn tool said
├── selftest/kept/            # gdd selftest: a file set aside while the check runs
├── upgrade/                  # gdd upgrade: scratch folder, removed after each run
├── bin/                      # protected; gdd does not write it
└── changes/<id>/
    ├── change.json           # the record
    ├── inbox.md              # the Decision Inbox
    ├── plan.md               # the plan of a red change (the Orchestrator)
    ├── package.md            # the minimum context package (the Orchestrator)
    ├── replan.md             # the last replan (the Orchestrator, headless)
    ├── review.md             # the last review (the Reviewer's text, kept by gdd)
    ├── g2.json  g3.json      # the last report of each gate
    ├── snapshot.json         # gdd snapshot
    ├── snapshot-build.json   # the snapshot taken after test mode
    ├── run.json              # while the change waits at G4
    ├── outputs/              # every agent call: <role>-<n>.md and <role>-<n>.raw
    └── demo/                 # what the demo step leaves for G4
```

| Ruta | Lo escribe | Contenido |
|---|---|---|
| `log.jsonl` | gdd | Consulta [log.jsonl](#logjsonl). |
| `g0.json` | `gdd gate G0` | `decision`, `by` (`machines`), `ts`, `note`. |
| `zones.json` | `gdd gate zones` | `decision`, `by` (`human`), `ts`, `red_hash` (16 caracteres hexadecimales del SHA-256 de las zonas rojas), `red` (las zonas rojas tal como se aprobaron, con todas sus claves), `note`. |
| `selftest.json` | `gdd selftest` | `ok`, `summary` (`harness 4/4`), `detail` (el `test` y el `ok` de cada comprobación), `ts`. |
| `headless_sessions.txt` | gdd, tras cada llamada de agente que informa de una sesión | Un id de sesión por línea, para que `gdd session` las deje fuera. |
| `imported_requests.json` | `gdd session` | Los ids de las peticiones ya contadas, como lista JSON. Un fichero `.new` guarda la lista siguiente hasta que el log tiene sus líneas. |
| `*.lock` | gdd | Ficheros de bloqueo vacíos: `merge.lock` (una fusión cada vez), `launch.lock` (el tope de `[parallel] max`), `agents.lock` (lo retiene cada llamada de agente, y en exclusiva la escritura de una persona en el estado compartido), `state.lock` (el turno para tomarlo). |
| `adoption/<command>.txt` | `gdd adopt` | Lo que se guarda de la salida del comando que falla, para su Refactor. |
| `learn/<tool>.txt` | `gdd learn` | Todo lo que dijo una herramienta de Learn. |
| `bin/` | no es gdd | Programas que un proyecto deja fuera de git, como su propia compilación de gdd. Está en `[protected] all` y en la zona roja del arnés, y se comprueba antes y después de cada llamada de agente. |
| `changes/<id>/change.json` | solo gdd | Consulta [change.json](#changejson). |
| `changes/<id>/inbox.md` | gdd, tras cada modificación del registro | Consulta [inbox.md](#inboxmd). |
| `changes/<id>/plan.md`, `package.md` | el Orchestrator (`gdd-change`) | Texto libre. El Builder y el Reviewer leen `package.md`. |
| `changes/<id>/replan.md` | el Orchestrator al replanificar | Texto libre; la última línea que empieza por `NEXT:` dice `test` o `build`. gdd lo borra antes de cada replanificación. |
| `changes/<id>/review.md` | gdd, a partir del texto final del Reviewer | Termina en `VERDICT: OK` o `VERDICT: KO`. gdd lo borra antes de cada revisión. |
| `changes/<id>/g2.json`, `g3.json` | `gdd g2`, `gdd g3`, la receta | Consulta [Informes de puerta](#informes-de-puerta). |
| `changes/<id>/snapshot.json` | `gdd snapshot` | Cada ruta que tocó el Change con su SHA-256 (o `deleted`): `gdd guard --since-snapshot` solo juzga lo que cambia después. |
| `changes/<id>/snapshot-build.json` | la receta, tras el modo test | Lo mismo, para que el modo build se juzgue solo por su propio trabajo. |
| `changes/<id>/run.json` | la receta | `{"paused_at": "G4"}` mientras el Change espera a `gdd resume`; se borra tras la respuesta. |
| `changes/<id>/outputs/` | gdd, tras cada llamada de agente | `<role>-<n>.md`: el texto final del agente; `<role>-<n>.raw`: su salida completa. `<role>` es `builder-test`, `builder-build`, `reviewer` u `orchestrator`; `<n>` cuenta las llamadas del rol en el Change, desde 1. |
| `changes/<id>/demo/` | el comando `[release] demo` del proyecto | Vídeo, capturas de pantalla o la ejecución legible de los escenarios. |

## change.json

El registro de un Change: qué planea, qué decidieron sus puertas, sus notas y su estado. JSON
indentado. Solo lo escribe gdd. Los comandos de gdd que puede ejecutar un agente (`gdd note`,
una puerta en modo build) solo cambian `inbox` y `out_of_scope`, y gdd deshace cualquier otra
modificación que haga un agente.

El registro de un Change fusionado y promovido en un proyecto desechable, recortado:

```json
{
  "build_start": "ca7bc9c0d5369ef863aab8d6f7651f64056215ed",
  "changelog": {
    "breaking": false,
    "line": "The cart shows its total",
    "section": "Added"
  },
  "commit": "8b14c1a9ab22d8931b010ad51499c8242ff03e4a",
  "created": "2026-10-30T13:11:40+00:00",
  "decision": {
    "builder": {
      "integration": "claude",
      "model": "sonnet"
    },
    "builder_session": "separate",
    ...
    "type": "Feature"
  },
  "features": [
    "features/cart.feature"
  ],
  "gates": {
    "G1": {
      "by": "human",
      "decision": "ok",
      "hashes": {
        "features/cart.feature": "06a581fd190d4dc2e48f33c21323302a67e522e74b8bacce3dd2b51c3fdce716"
      },
      "note": "approved at the sitting",
      "ts": "2026-10-30T13:11:46+00:00"
    },
    ...
  },
  "id": "cart-total",
  "inbox": [
    {
      "kind": "decided",
      "text": "Kept the total as an integer",
      "ts": "2026-10-30T13:11:53+00:00"
    },
    ...
  ],
  "min_color": null,
  "out_of_scope": [
    "docs/extra.md"
  ],
  "paths": [
    "src/shop/**",
    "features/cart.feature"
  ],
  "resumable": false,
  "state": "closed",
  "title": "Show the cart total",
  "type": "Feature",
  "version": "0.1.0"
}
```

| Campo | Tipo | Lo escribe | Significado |
|---|---|---|---|
| `id` | cadena | `gdd new-change` | El id del Change: una letra minúscula o un dígito, y después hasta 40 letras minúsculas, dígitos o guiones. Su rama es `change/<id>`. |
| `title` | cadena | `gdd new-change` | De qué trata el Change. |
| `type` | cadena | `gdd new-change` | `Feature`, `Hotfix`, `Refactor`, `Migration`, `Spike` o `No-code`. |
| `paths` | array de cadenas | `gdd new-change` | Las rutas que planea tocar; se admiten patrones. |
| `resumable` | booleano | `gdd new-change` | `--resumable`: una condición de la vía rápida. |
| `min_color` | cadena o null | `gdd new-change` | `--min-color`: sube el color, nunca lo baja. |
| `created` | cadena | `gdd new-change` | Cuándo se creó, en UTC. |
| `state` | cadena | gdd | Consulta [Estados](#estados). |
| `gates` | objeto | `gdd gate`, la receta | Una entrada por cada puerta decidida en el Change: ver más abajo. |
| `inbox` | array | `gdd note`, `gdd park`, la receta | Las notas de la Decision Inbox: ver más abajo. |
| `variants` | array | `gdd new-change`, la receta | Solo en un Spike con variantes: ver más abajo. |
| `decision` | objeto | `gdd prepare` | Lo que decidió prepare: ver más abajo. |
| `build_start` | cadena | `gdd prepare` | El commit en el que empezó el Build: Path Guard y la comprobación de alcance juzgan a partir de él. |
| `features` | array de cadenas | `gdd gate G1` | Los ficheros de escenarios sellados en G1, incluidos los ejemplos ocultos. |
| `out_of_scope` | array de cadenas | las puertas en modo build | Las rutas fuera del plan que aceptó la comprobación de alcance. |
| `reviews` | array | la receta | Cada llamada al Reviewer: ver más abajo. |
| `spent_usd` | número | la receta | Lo que costaron las llamadas de agente del Change, según lo informaron los agentes. |
| `spent_tokens` | objeto | la receta | Sus tokens: `in`, `cache`, `out`. |
| `changelog` | objeto | `gdd changelog-line` | `{"section", "line", "breaking"}`, o `{"none": true}` para dejar el Change fuera. Sin él, el título va en la sección de su tipo (Feature: Added, Hotfix: Fixed, Migration: Changed); los demás tipos no tienen línea. |
| `commit` | cadena | `gdd merge` | El commit de fusión. |
| `version` | cadena | `gdd release promote` | La versión con la que se publicó el Change, sin su `v`. |

Los registros escritos por una versión anterior de la herramienta pueden tener otros campos;
gdd los ignora.

### gates

| Clave | Presente en |
|---|---|
| `plan` | un Change rojo, una vez que la persona aprobó su plan |
| `G1` | un Change con escenarios nuevos o cambiados, una vez que la persona decidió |
| `G2` | cuando se registra con `gdd gate G2` |
| `G3` | una vez que las máquinas la abrieron (`automatic chain and Reviewer`) |
| `G4` | una vez que la persona respondió, o que las máquinas aprobaron la promoción |

Cada entrada tiene `decision` (`ok` o `ko`), `by` (`human` o `machines`), `ts` y `note`. `G1`
tiene además `hashes`: cada fichero de escenarios sellado y su SHA-256.

### inbox

Cada nota tiene `kind`, `text` y `ts`, y además `resolved` (cuándo se resolvió) una vez que un
relanzamiento la resolvió. `kind` es `needs` (una decisión para la persona), `decided` (tomada
sin ella), `parked` (por qué se detuvo) o `polish` (las notas de la persona en G4).

### decision

La clasificación del Change cuando se preparó, y el modelo de cada rol.

| Clave | Significado |
|---|---|
| `change` | El id. |
| `color`, `lane` | `green`, `yellow` o `red`; `fast` o `full`. |
| `reasons` | Una línea por ruta: su color y el porqué. |
| `g1`, `features` | Si necesita G1, y sus ficheros de escenarios. |
| `tests_first`, `mutation`, `reviewer`, `review`, `g4`, `human_plan` | Lo que le da su regla (consulta [Puertas](gates.md#puertas-por-color)). |
| `type` | Su tipo de trabajo. |
| `builder`, `builder_test`, `reviewer_model`, `orchestrator` | Las tablas de rol de `gdd/models.toml` que le tocaron. |
| `builder_session` | `separate` o `shared`. |
| `retries` | Rondas del modo build contra G2. |
| `resume_green` | Un relanzamiento cuyo código ya pasa G2: sin Builder. |

### reviews

Una entrada por cada llamada al Reviewer, en orden, incluidos los relanzamientos.

| Clave | Significado |
|---|---|
| `output` | Dónde se guarda su respuesta, desde la carpeta del Change: `outputs/reviewer-<n>.md`. |
| `verdict` | `OK` o `KO` cuando terminó la llamada; `null` cuando la llamada detuvo la receta. |
| `changed` | Las rutas cuyo hash cambió desde la revisión anterior; `null` en la primera. |
| `hashes` | El hash de cada ruta que tocó el Change, justo antes de la llamada. |

### variants

Una entrada por cada variante de un Spike: `option` (la opción que construye, o `null` para el
mismo brief), `branch` (`change/<id>-v<n>`) y, una vez construida, `state` (`ready` o
`parked`), `reason`, `g2`, `adrs`, `decision` (la primera línea de la decisión de su ADR),
`files`, `minutes`, `usd` y `tokens`.

### Estados

| Estado | Significado |
|---|---|
| `open` | Creado, sin preparar. |
| `building` | Preparado y en Build, incluida una pasada de polish tras G4. |
| `parked` | Detenido, con un mensaje en su Decision Inbox. |
| `merged` | Fusionado en main. |
| `in-pre` | En pre, esperando G4 o su promoción. |
| `promoting` | Su promoción está en curso. |
| `in-production` | Promovido, en observación. |
| `closed` | Observado: terminado. |
| `rejected` | KO en G4. |
| `rolled-back` | La observación falló y se revirtió producción. |

## inbox.md

La Decision Inbox: un mensaje por Change que reúne lo que decidieron los agentes y lo que te
espera. gdd la vuelve a generar a partir del registro y de los informes de puerta cada vez que
cambian. `gdd inbox <id>` la imprime.

```text
# Decision Inbox · cart-total · Show the cart total

Color yellow · lane full · state closed

## Needs your decision
- Should the total show taxes

## Decided without you
- Kept the total as an integer
- Paths outside the package, same or lower color, accepted by rule: docs/extra.md

## Parked
- Nothing.
```

`Needs your decision` es el único grupo que te espera. Los grupos, en orden, cada uno solo
cuando tiene algo (los tres grupos de notas aparecen siempre): `How it works` (del OK del
Reviewer), `Variants`, `Needs your decision`, `Decided without you`, `Parked`,
`Security exceptions this change adds`, `Known debt`, `Known debt paid`, `Polish`; después,
cuántas peticiones anteriores se resolvieron antes del último lanzamiento.

## Informes de puerta

`g2.json` y `g3.json` guardan la última ejecución de cada puerta en el Change.

```json
{
  "ok": true,
  "gate": "G2",
  "color": "yellow",
  "passed": [
    "lint",
    "unit",
    "path-guard",
    "scope"
  ],
  "skipped": [],
  "failures": [],
  "out_of_scope": [
    "docs/extra.md"
  ]
}
```

| Campo | Significado |
|---|---|
| `ok` | No falló nada. |
| `gate`, `color` | `G2` o `G3`; el color del Change. |
| `passed` | Las comprobaciones que pasaron, en orden: comandos, `path-guard`, `scope`, `scenarios (<n>)`, `exceptions`. |
| `skipped` | Los comandos `mutation*` que el Change no necesita. |
| `failures` | Cada uno con `name` y `tail` (lo que hay que leer); un comando tiene además `ok`, `code` (124 tras agotar el tiempo) y `duration_s`. |
| `out_of_scope` | En modo build, las rutas fuera del plan que aceptó la comprobación de alcance. |
| `added_exceptions` | Solo en G3, si las hay: las excepciones de seguridad que añade el Change. |
| `known_debt` | Si los hay: los comandos con deuda conocida que fallaron, con `name`, `tail`, y `count` y `known` cuando se cuentan. |
| `paid_debt` | Si los hay: los comandos con deuda conocida que pasaron. |
| `summaries` | Si los hay: la última línea de cada comando `mutation*` que pasó. |

## log.jsonl

El log del que salen todos los informes: un objeto JSON por línea, añadido bajo un lock, para
que las líneas de dos ejecuciones nunca se mezclen. gdd escribe una línea por cada llamada de
agente, decisión de puerta, comando de puerta, ejecución de Path Guard y paso de máquina.
`gdd session` añade las sesiones interactivas de Claude Code. Los informes dejan fuera las
líneas que no son de gdd (no son JSON, o su coste no es un número mayor o igual que cero) e
indican cuántas son.

```json
{"ts":"2026-10-30T13:11:46+00:00","change":"cart-total","phase":"2-build","gate":"G2","role":"machine","agent":"lint","model":"-","tokens_in":0,"tokens_cache":0,"tokens_out":0,"cost_usd":0.0,"duration_s":0.0,"result":"ok","retry":0,"note":""}
{"ts":"2026-10-30T13:11:46+00:00","change":"cart-total","phase":"2-build","gate":"G2","role":"machine","agent":"checks","model":"-","tokens_in":0,"tokens_cache":0,"tokens_out":0,"cost_usd":0.0,"duration_s":0.0,"result":"ok","retry":0,"note":""}
```

La primera línea es el comando `lint` de G2; la segunda, la puerta entera (`checks`).

| Campo | Tipo | Significado |
|---|---|---|
| `ts` | cadena | UTC al segundo: `2026-10-30T13:11:46+00:00`. |
| `change` | cadena | El id del Change; `-` para el proyecto (G0, las zonas rojas). |
| `phase` | cadena | `0-setup`, `1-discover`, `2-build`, `3-integrate`, `4-release`, `5-learn` o `-`. |
| `gate` | cadena | `G0`, `zones`, `plan`, de `G1` a `G4`, o `-`. |
| `role` | cadena | `orchestrator`, `builder-test`, `builder-build`, `reviewer`, `machine`, `human`, o quién registró una puerta (`--by`, como `machines`). |
| `agent` | cadena | El agente de una llamada (`claude`, `codex`, `opencode`, `claude-interactive`); en los pasos de máquina, lo que se ejecutó: un nombre de comando, `checks` (la puerta), `path-guard`, `selftest`, `verdict`, `merge`, un paso de release; `-` en otro caso. |
| `model` | cadena | El modelo de una llamada de agente; `-` en otro caso. |
| `tokens_in` | entero | Tokens de entrada no leídos de la caché. |
| `tokens_cache` | entero | Tokens de entrada leídos de la caché. |
| `tokens_out` | entero | Tokens de salida. |
| `cost_usd` | número | Coste de la llamada: el que informó el agente, o estimado a partir de `[prices]`. |
| `duration_s` | número | Segundos, con una décima. |
| `result` | cadena | `ok` o `ko`; en una llamada de agente, `ok`, `error-<code>` o `timeout`; en un paso de release, `ok`, `ko` o `simulated`; en un paso, lo que pasó: `change-created`, `prepared`, `parked`, `changelog-line`, `released`, `polish`. |
| `retry` | entero | En una llamada de agente, las llamadas anteriores de su rol en el Change; 0 en otro caso. |
| `note` | cadena | Lo que hay que leer: las comprobaciones fallidas de una puerta, la nota de una decisión, el comando que ejecutó un agente (`/gdd-build`), la versión publicada. |

## El commit de fusión

`.gdd/` nunca sale de tu máquina, así que `gdd merge` lleva a git lo importante del registro:
fusiona un Change en la rama principal con un commit cuyo mensaje dice qué era el Change y quién
lo aprobó. El commit de fusión del mismo Change, recortado:

```text
gdd(cart-total): G3 Merge Gate · Show the cart total

Color: yellow · lane full · type Feature
Approvals: G1 ok (human, 2026-10-30, 1 scenario file(s) sealed)
Decided without the human:
  - Kept the total as an integer
...
Record: .gdd/changes/cart-total/ (local)

Changelog: Added · The cart shows its total
Breaking: no
```

| Línea | Cuándo |
|---|---|
| `gdd(<id>): G3 Merge Gate · <title>` | Siempre. |
| `Color: <color> · lane <lane> · type <type>` | Siempre. |
| `Approvals: ...` | Las zonas rojas (Changes rojos), G1 y el plan cuando el registro los tiene; si no, `Approvals: none needed`. |
| `Reviewer: KO -> OK`, y después sus hallazgos | Cuando el Change pasó por revisión. |
| `Decided without the human:` | Las ocho últimas notas `decided`. |
| `Tokens: in <n> · cache <n> · out <n> · <usd> USD (estimate) · <n> agent call(s)` | Cuando el log tiene llamadas del Change. |
| `Known debt paid: ...`, `Known debt lowered: ...` | Cuando el Change pagó o redujo deuda conocida. |
| `Record: .gdd/changes/<id>/ (local)` | Siempre. |
| `Changelog: <section> · <line>` y `Breaking: yes` o `Breaking: no` | Trailers de git, en un párrafo propio, cuando el Change tiene línea de changelog. |

`gdd changelog` y `gdd release` leen los trailers `Changelog:` y `Breaking:` de los commits de
fusión de main para escribir `CHANGELOG.md` y elegir la siguiente versión.

## Ver también

- [Configuración](configuration.md): los ficheros de `gdd/`.
- [Variables de entorno](environment.md): lo que reciben los comandos y los agentes que
  escriben estos ficheros.
- [Volver a poner en marcha un Change aparcado](../how-to/parked-change.md): cómo leer la
  Decision Inbox.
- [Medir el trabajo](../how-to/measure.md): informes construidos a partir de `log.jsonl`.
