# Roles

Quién hace qué en un proyecto GDD: la persona, el Orchestrator, el Builder y el Reviewer. Para
cada uno: qué lee, qué puede escribir, qué no debe tocar, qué modelo lo desempeña y qué
comandos ejecuta.

## Resumen

| Rol | Lo desempeña | Lo lanza | `GDD_ROLE` | Lista de Path Guard |
|---|---|---|---|---|
| [La persona](#la-persona) | Una persona | | sin definir | ninguna |
| [El Orchestrator](#el-orchestrator) | Un agente, modelo fuerte | La persona (`gdd-status`, `gdd-change`...), o la receta para una replanificación | `orchestrator` en una replanificación | ninguna propia |
| [El Builder, modo test](#modo-test) | Un agente, según el color | La receta | `builder-test` | `all` + `builder_test` |
| [El Builder, modo build](#modo-build) | Un agente, según el color | La receta | `builder-build` | `all` + `builder_build` |
| [El Reviewer](#el-reviewer) | Un agente de otra familia de modelos | La receta | `reviewer` | `all` + `reviewer` |
| Las máquinas | `gdd` y los comandos de las puertas | | | |

Las máquinas solo dicen sí o no, y por qué: ejecutan las puertas, Path Guard y los pasos de
release.

gdd rechaza estos comandos cuando `GDD_ROLE` empieza por `builder` o `reviewer`: `gdd gate`,
`gdd resume`, `gdd release`, `gdd changelog-line`, `gdd selftest`, `gdd learn` y `gdd upgrade`.
Una ejecución de `gdd g2` o `gdd g3` con cualquier `GDD_ROLE` no se registra.

Antes y después de cada llamada de agente de la receta, gdd comprueba dos cosas, y cualquiera
de ellas que falle aparca el Change:

- Ningún agente ha cambiado el estado de GDD, ni el de su Change ni el compartido (`.gdd/`,
  enlaces incluidos), ni ha dejado una carpeta `.gdd` en su worktree. Se permite lo que
  escriben los comandos de gdd que puede ejecutar (notas, las rutas que ha aceptado la
  comprobación de alcance, su informe de puerta); cualquier otra cosa se restaura.
- El Reviewer no ha cambiado nada: el árbol de trabajo fuera de `.gdd/` es el mismo después de
  su llamada, hasta en el tipo de cada ruta, sus permisos y adónde apuntan sus enlaces.

## La persona

El stakeholder. No lee código; decide lo que solo una persona puede decidir, en el menor número
de sentadas posible.

**Lee:** el Project Brief (`docs/brief.md`), los escenarios de un Change (no los ejemplos
ocultos), las zonas rojas y sus motivos, el plan de un Change rojo, la Decision Inbox
(`.gdd/changes/<id>/inbox.md`), la demo y la página de G4 (`gdd sitting`, el Board).

**Decide:**

| Parada | Cómo |
|---|---|
| Las respuestas del Setup (fase 0) y Discover (fase 1) | En conversación con el Orchestrator. |
| La sentada de un Change: escenarios (G1), zonas rojas, plan de un Change rojo | `gdd sitting <id>`, o el Orchestrator registra la respuesta con `gdd gate G1 ok <id>`, `gdd gate zones ok` y `gdd gate plan ok <id>`, cada uno con `--by human`. |
| G4 | `gdd resume <id> --g4 ok`, `--g4 ko` o `--g4 polish --note "..."`. |
| Un Change aparcado | Lee su Decision Inbox; un mensaje por Change. |

**Escribe:** a través del Orchestrator, el arnés en la rama principal (`gdd/*.toml`, `bin/`,
`.github/`, las carpetas de los agentes); `docs/standards.md`, el listón de calidad del
proyecto. Path Guard no juzga a la persona.

**Ejecuta:** `gdd init`, `gdd bootstrap`, `gdd agents install`, `gdd upgrade` (y luego fusiona
la rama `gdd-upgrade`), `gdd release` y `gdd changelog`, y, con el Orchestrator, los comandos
de agente `gdd-status`, `gdd-setup`, `gdd-discover`, `gdd-change`, `gdd-learn`, `gdd-hotfix` y
`gdd-report`.

Paradas humanas por Change: 0 en la vía rápida (*Fast Lane*), de 0 a 2 en la vía completa
(*Full Lane*), 2 para un Change rojo (la sentada y luego G4), 0 o 1 para un Hotfix; como mucho,
un mensaje por Change.

## El Orchestrator

Tiene el mapa: arquitectura, ADR, zonas, contratos, escenarios, estado y registro. Trabaja con
la persona (de forma interactiva), y sin él una vez por cada Change atascado (replanificación
desatendida, *headless*).

**Lee:** `gdd/project.toml`, `docs/brief.md`, `bin/gdd status`, `bin/gdd report`, los
expedientes y las Decision Inbox de `.gdd/changes/`, y, para una replanificación, el paquete,
`g2.json` y las salidas del Builder.

**Escribe:**

| Qué | Dónde | Cuándo |
|---|---|---|
| Project Brief, ADR de tecnología, esqueleto, comandos de las puertas, CI | `docs/brief.md`, `docs/adr/0001-technology.md`, `gdd/`, `.github/` | Setup, en main |
| Notas de Discover, ADR | `docs/discover/YYYY-MM-DD-<topic>.md`, `docs/adr/` | Discover |
| Mapa de zonas | `gdd/zones.toml`, en main | Al final de Discover; se aprueba en la siguiente sentada |
| Un Change | `bin/gdd new-change <id> ...` | Change |
| Escenarios y ejemplos ocultos | `features/`, `features/hidden/`, en la rama del Change | Antes de G1 |
| Plan de un Change rojo | `.gdd/changes/<id>/plan.md` | Antes de la sentada |
| Paquete de contexto mínimo | `.gdd/changes/<id>/package.md` | Antes del lanzamiento |
| Decisiones tomadas, preguntas | `bin/gdd note <id> decided "..."`, `bin/gdd note <id> needs "..."` | En cualquier momento |
| Replanificación | `.gdd/changes/<id>/replan.md`, terminada en `NEXT: test` o `NEXT: build` | Desatendida, con G2 todavía en rojo |
| Propuestas de Learn | `docs/learn/YYYY-MM-DD.md` | Learn |

**No debe:** escribir código de producto (fuera de un Spike), resolver conflictos de fusión,
relajar una puerta, editar escenarios aprobados ni cambiar `gdd/` sin la persona. En una
replanificación, del estado de GDD solo escribe `replan.md`; la ficha de Claude Code además le
deniega `.gdd/log.jsonl` y todos los `change.json`.

**Ejecuta:** en una replanificación, `bin/gdd status`, `classify`, `note`, `report` y lecturas
de git.

**Comandos:** `gdd-status`, `gdd-setup`, `gdd-discover`, `gdd-change`, `gdd-learn`,
`gdd-hotfix`, `gdd-report` (con la persona); `gdd-replan` (desatendido,
`change=<id> reason=g2`).

**Modelo:** `[orchestrator]` de `gdd/models.toml`; `gdd init` le asigna el modelo fuerte del
primer agente.

## El Builder

Escribe los tests y el código de un Change, en dos modos, siempre en llamadas separadas.
Trabaja en el worktree del Change, sin credenciales de producción (consulta [Variables de
entorno](environment.md#qué-reciben-los-agentes)) y sin preguntar a nadie: nadie responde.

Los dos modos:

- leen las instrucciones de cada stack del proyecto (`gdd-stack-<name>`) y su paquete,
  `<state>/.gdd/changes/<id>/package.md`;
- comprueban su trabajo con `bin/gdd g2 --role <their role>`, que es lo que cuenta;
- solo pueden ejecutar `bin/gdd g2`, `bin/gdd guard`, `bin/gdd note` y lecturas de git
  (`git status`, `git diff`, `git log`, `git show`), cada comando por separado;
- nunca editan tests ni ficheros `.feature` aprobados, ni tocan rutas protegidas, ni silencian
  avisos o se saltan comprobaciones, ni escriben casos especiales para los ejemplos, ni añaden
  una dependencia sin anotarla;
- anotan lo que han decidido con `bin/gdd note <id> decided "..."`, y lo que necesita a la persona
  con `bin/gdd note <id> needs "..."`.

**Modelo:** `[builder.<color>]` de `gdd/models.toml` para el modo build, según el color del
Change; `[builder_test]` para el modo test, en todos los colores. Barato o local en verde, más
capaz en rojo.

### Modo test

**Comando:** `gdd-build-test`, con `change=<id>`, más `mode=test` (o `mode=test after=replan`)
desde la receta. `GDD_ROLE=builder-test`.

**Lee:** el paquete, los ficheros `.feature` que nombra y el código bajo las rutas del paquete
(solo para leerlo). Tras una replanificación, `replan.md`.

**Escribe:** las definiciones de pasos de los escenarios, en el lenguaje del dominio y con
aserciones reales; tests de caracterización antes de tocar código amarillo o rojo ya existente.
Los tests nuevos deben fallar porque falta el comportamiento.

**No debe tocar:** `[protected] all` y `builder_test`: el código de producción (`src/**` con
las fichas de `gdd init`; también `crates/*/src/**` en Rust).

Se ejecuta cuando el Change necesita primero los tests, antes del modo build, y otra vez cuando
una replanificación dice `NEXT: test`. Después se ejecuta Path Guard: un fallo aparca el
Change.

### Modo build

**Comando:** `gdd-build`, con `change=<id>` y uno de estos: `mode=build`, `mode=build fix=g3`,
`mode=build fix=review`, `mode=build fix=polish`, `mode=build after=replan`, o `mode=spike` (y
`variant=<n>`) para un Spike. `GDD_ROLE=builder-build`.

**Lee:** el paquete; `docs/standards.md`, si existe; el último `g2.json`; con `fix=g3`,
`g3.json`; con `fix=review`, `review.md`; con `after=replan`, `replan.md`; con `fix=polish`,
las notas Polish de `inbox.md`; el código bajo las rutas del paquete.

**Escribe:** el dominio, el cableado y los tests unitarios, hasta que G2 esté en verde. Un
Spike escribe lo mínimo que responde a su pregunta y un ADR en `docs/adr/` con una sección
`## Decision`.

**No debe tocar:** `[protected] all` y `builder_build`: los escenarios aprobados y los pasos
escritos en modo test (`features/**` y la carpeta de pasos del stack). Una ruta fuera del
paquete, del color del Change o inferior, se acepta y se anota; una de color más alto hace
fallar la comprobación de alcance.

## El Reviewer

Revisa el Change entero después de G3, desde una familia de modelos distinta de la del Builder,
para no compartir sus puntos ciegos. Responde OK o KO con motivos; hay una sola corrección; no
abre rondas nuevas.

**Comando:** `gdd-review`, con `change=<id> round=1` o `round=2`. `GDD_ROLE=reviewer`.

**Lee:** el paquete, `change.json` (el color, las puertas, `review` en `decision`, las
revisiones anteriores en `reviews`), los ficheros `.feature` del Change, el diff completo
(`git diff main...HEAD`), `g2.json`, `g3.json`, `docs/standards.md` y las instrucciones del
stack.

**Comprueba:** que cada paso hace lo que dice su escenario, con aserciones reales; que no hay
casos especiales para los ejemplos; que el Change no sale de su paquete; en rojo, la seguridad;
el trabajo que solo parece terminado; un paso nuevo que repite uno existente. La revisión
rápida lee los ficheros que ha tocado el Change, el paquete y los escenarios; la profunda,
también el código de alrededor. Cada hallazgo lleva un id, una clase de bug, `file:line` y
todos los demás sitios donde aparece esa clase.

**Escribe:** nada. gdd guarda su texto final como `.gdd/changes/<id>/review.md` y
`outputs/reviewer-<n>.md`. Un OK termina con una sección `## How it works`, que se muestra la
primera en la página de G4 y en la Decision Inbox. La última línea es `VERDICT: OK` o
`VERDICT: KO`; sin un veredicto legible, cuenta como KO.

**No debe tocar:** nada: `[protected] reviewer` es `["**"]`, se ejecuta en solo lectura y gdd
aparca el Change si su árbol de trabajo ha cambiado.

**Ejecuta:** solo lecturas de git.

**Modelo:** `[reviewer]` de `gdd/models.toml`. `gdd prepare` rechaza un Change cuyo Reviewer es
de la familia del Builder, salvo que `same_family_accepted` registre el día en que la persona lo
aceptó.

## Modelos por defecto

`gdd init` rellena `gdd/models.toml` a partir de `--agents` (el primero construye y orquesta) y
de los valores por defecto de cada agente. El Reviewer es el primero de los demás agentes que
sea de otra familia; con una sola familia, `gdd init` necesita `--accept-same-family` y el
Reviewer vuelve a ser el primer agente.

| Rol | Modelo |
|---|---|
| `[orchestrator]` | fuerte, del primer agente |
| `[builder.red]` | fuerte, del primer agente |
| `[builder.yellow]` | estándar, del primer agente |
| `[builder.green]` | ligero, del primer agente |
| `[builder_test]` | estándar, del primer agente |
| `[reviewer]` | fuerte, del agente del Reviewer, con su esfuerzo |

| Agente (`integration`) | Familia | Fuerte | Estándar | Ligero | Esfuerzo del Reviewer |
|---|---|---|---|---|---|
| `claude` (Claude Code) | `anthropic` | `opus` | `sonnet` | `haiku` | ninguno |
| `codex` (Codex) | `openai` | `gpt-6-astra` | `gpt-6-astra` | `gpt-6-astra` | `high` |
| `opencode` (OpenCode) | `minimax` | `minimax/MiniMax-M3` | `minimax/MiniMax-M3` | `minimax/MiniMax-M3` | ninguno |

Con `--agents claude codex`: Claude Code construye y orquesta (opus, sonnet, haiku) y Codex
revisa (`gpt-6-astra`, esfuerzo `high`).

## Permisos de los agentes

Las fichas de agente dan a cada rol sus argumentos. Ningún rol recibe servidores MCP. Cada uno
se ejecuta en el worktree del Change y solo puede escribir en la carpeta de estado de su propio
Change a través de `bin/gdd`.

| Rol | Claude Code | Codex | OpenCode |
|---|---|---|---|
| Builder, los dos modos | `--permission-mode acceptEdits`; permitido: leer, editar, escribir, `bin/gdd g2`, `guard`, `note`, lecturas de git, `ls`; denegado: `bin/gdd gate`, `launch`, `resume`, `merge`, `release`, `git commit`, `switch`, `checkout`, `push`, la web, editar `.gdd/` | `--sandbox workspace-write`, más la carpeta de estado de su Change | todavía sin flags |
| Reviewer | `--permission-mode plan`; permitido: leer y lecturas de git; las mismas denegaciones | `--sandbox read-only` | todavía sin flags |
| Orchestrator (replanificación) | `--permission-mode acceptEdits`; permitido: leer, editar, escribir, `bin/gdd status`, `classify`, `note`, `report`, lecturas de git; denegado: editar `.gdd/log.jsonl` y cualquier `change.json` | `--sandbox workspace-write`, más la carpeta de estado de su Change | todavía sin flags |

Las comprobaciones propias de gdd (el estado de GDD y un Reviewer en solo lectura) se aplican a
todos los agentes, OpenCode incluido.

## Ver también

- [Mantener honestos a los agentes](../explanation/safety.md): por qué existen Path Guard, los
  ejemplos ocultos y las comprobaciones del estado.
- [Puertas](gates.md): lo que tiene que pasar el trabajo de cada rol.
- [Elige agentes y modelos](../how-to/agents-and-models.md): cambia quién desempeña cada rol.
- [models.toml](configuration.md#modelstoml): todas las claves del fichero de modelos.
