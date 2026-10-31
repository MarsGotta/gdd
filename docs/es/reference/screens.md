# Pantallas

gdd tiene tres pantallas en la terminal: el formulario de Setup de `gdd init`, el Board y la
sentada, que tiene una segunda página para la puerta de salida (G4, *Release Gate*). Esta página
muestra cada una, sus teclas y lo que escribe cada tecla. Consúltala cuando respondas en una
pantalla en vez de en el chat con tu agente.

<!-- Every capture on this page is a real screen of gdd 0.1.0 in a terminal of 100 x 30, from a
     throwaway project, with the blank lines trimmed; the G4 sitting is the one of the
     tutorial's real run, from how-to/release-gate.md. The form's summary and output, the empty
     Board and the sitting with nothing waiting were captured again after gdd 0.1.0 (commit
     07b1cd6), in a project called shop. -->

## Antes de empezar

Cada pantalla necesita una terminal y una persona. Si la ejecutas desde un script o una tubería,
el comando se niega y dice qué ejecutar en su lugar; los mensajes están bajo cada pantalla. Un
agente que ha lanzado gdd nunca abre ninguna (consulta
[Roles](roles.md#qué-puede-ejecutar-un-agente)). `Ctrl-C` cierra cualquiera de ellas en cualquier
momento.

## El formulario de Setup

El formulario de Setup prepara un proyecto a partir de tus respuestas en vez de flags.
[`gdd init`](cli/init.md) lo abre en una terminal cuando no le das `--platform`. Hace siete
preguntas, una por paso, muestra un resumen y no escribe nada hasta que pulsas `Enter` en
el resumen. Los flags `--agents`, `--language`, `--description` y `--sensitive`, cuando los das,
llegan ya rellenos; el nombre, la plataforma y los stacks se preguntan otra vez.

```bash
gdd init
```

```text
┌ gdd init · Step 1 of 7 · Project name ───────────────────────────────────────────────────────────┐
│What is the project called?                                                                       │
│> █                                                                                               │
│Enter: next · Ctrl-C: quit                                                                        │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Paso | Pregunta | Respondes con |
|---|---|---|
| 1 · Project name | What is the project called? | texto; `Enter` se queda aquí hasta que escribes un nombre |
| 2 · Description | What is it, in a few words? It may stay empty. | texto |
| 3 · Platform | Where does it run? | `↑` `↓` hasta `mobile`, `desktop`, `web`, `cli` u `other`, después `Enter` |
| 4 · Stacks | Which technology does it use? | `Space` marca `rust`, `typescript`, `python` o `Not sure` |
| 5 · Coding agents | Which coding agents? The first checked builds. | `Space` marca `claude`, `codex`, `opencode` |
| 6 · Language | Which language do the agents use with you? | texto, `en` por defecto |
| 7 · Sensitive areas | What would be serious if it failed? One area per line. | texto, `Enter` después de cada área |

Los pasos con una lista muestran una casilla por elemento; `>` marca el cursor:

```text
┌ gdd init · Step 4 of 7 · Stacks ─────────────────────────────────────────────────────────────────┐
│Which technology does it use? Pick one or more, or Not sure.                                      │
│  [ ] rust                                                                                        │
│  [ ] typescript                                                                                  │
│> [x] python                                                                                      │
│  [ ] Not sure                                                                                    │
│Up/Down: move · Space: check · Enter: next · Esc: back · Ctrl-C: quit                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Los agentes de código que hay en tu `PATH` llegan marcados. Desmarca los que no quieras; al
menos uno debe quedar. Las áreas sensibles se convierten en líneas del Brief, que la skill
`gdd-discover` convierte después en zonas rojas:

```text
┌ gdd init · Step 7 of 7 · Sensitive areas ────────────────────────────────────────────────────────┐
│What would be serious if it failed? One area per line.                                            │
│  - Losing notes                                                                                  │
│> A note leaking to another user█                                                                 │
│Enter: add, or go on when empty · Backspace: edit the last · Esc: back · Ctrl-C: quit             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Teclas

| Tecla | Qué hace |
|---|---|
| cualquier letra, `Space`, `Backspace` | escribe o borra, en los pasos que piden texto |
| `↑` `↓` | mueve el cursor, en los pasos con una lista |
| `Space` | marca o desmarca el elemento del cursor (stacks, agentes de código) |
| `Enter` | pasa al paso siguiente cuando este tiene respuesta; si no, dice en rojo lo que aún le falta y se queda. En el paso 7 añade el área escrita, y desde una línea vacía sigue adelante |
| `Backspace` en una línea vacía del paso 7 | recupera la última área, para editarla |
| `Esc` | vuelve un paso atrás y conserva todas las respuestas |
| `Ctrl-C` | sale: no se escribe nada |

Lo que dice `Enter` cuando un paso no tiene respuesta: `Type a name: the project needs one`,
`Pick at least one stack, or Not sure` y `Pick at least one coding agent`.

### El resumen

El resumen muestra todas las respuestas antes de escribir nada:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Name: shop                                                                                        │
│Description: A small web shop: a catalog, a cart and a checkout.                                  │
│Platform: web                                                                                     │
│Stacks: python                                                                                    │
│Agents: claude, codex                                                                             │
│Language: en                                                                                      │
│Sensitive: Payments and customer data.                                                            │
│Enter to set up · Esc to go back                                                                  │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` prepara el proyecto, igual que lo haría `gdd init` con las mismas respuestas como flags:
escribe el arnés, el Brief, el esqueleto de cada stack y las skills de cada agente de código, y
los guarda en un solo commit, `gdd init 0.1.0`. Después ejecuta la primera instalación de cada
stack, como hace [`gdd bootstrap`](cli/bootstrap.md), y guarda los ficheros de lock en un segundo
commit, `gdd bootstrap`. Enumera cada fichero que ha escrito, después lo que imprimió la
instalación, y termina enviándote a tu agente de código:

```text
gdd/project.toml
gdd/commands.toml
gdd/rules.toml
gdd/zones.toml
gdd/models.toml
gdd/generated.toml
docs/brief.md
.github/workflows/gdd.yml
AGENTS.md
CLAUDE.md
...
.claude/skills/gdd-setup/SKILL.md
.claude/skills/gdd-status/SKILL.md
.claude/skills/gdd-stack-python/SKILL.md
...
.agents/skills/gdd-stack-python/SKILL.md
python: uv lock && uv sync --locked
Using CPython 3.13.15
Resolved 40 packages in 15ms
...
Committed: uv.lock
Next: start your coding agent here and run the gdd-setup command (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

Cuando una instalación falla, el commit del arnés se queda: el formulario dice qué stack falló y
que `gdd bootstrap` vuelve a ejecutar la instalación cuando hayas arreglado la causa.

Cuando todos los agentes de código que marcaste son de una sola familia de modelos, el Reviewer
sería de la misma familia que el Builder, algo que GDD evita. El resumen pide `y` en vez de
`Enter`, y solo `y` prepara el proyecto:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Name: shop                                                                                        │
│Description: none                                                                                 │
│Platform: web                                                                                     │
│Stacks: rust                                                                                      │
│Agents: claude                                                                                    │
│Language: en                                                                                      │
│Sensitive: none                                                                                   │
│The Reviewer would be of the same model family as the Builder: press y to accept it               │
│Esc to go back                                                                                    │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Cuando marcas `Not sure` entre los stacks, `Enter` no escribe nada. gdd imprime el comando
`gdd init` con todas tus demás respuestas como flags, para que lo ejecutes cuando sepas la
tecnología:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Not sure of the technology yet: once you know it, run                                             │
│gdd init --name notes --platform cli --agents claude codex --language en --stack <pick one>       │
│Your coding agent can help you choose it.                                                         │
│Enter to finish · Esc to go back                                                                  │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Ctrl-C` en cualquier paso termina con esta línea y el código de salida 1:

```text
gdd init stopped: nothing was written
```

Sin terminal:

```text
error: gdd init opens its Setup form only in a terminal; else give it the flags --name, --platform and --stack
```

## El Board

[`gdd board`](cli/board.md) muestra todos los Changes del proyecto por fase. Solo lee: ninguna
tecla escribe nada. `gdd` sin comando también lo abre, cuando lo ejecuta una persona en una
terminal; en cualquier otro sitio, y para un agente, `gdd` a secas imprime la ayuda.

```bash
gdd board
```

```text
┌ gdd · Board · notes ─────────────────────────────────────────────────────────────────────────────┐
│Discover (2)                                                                                      │
│› export-csv · yellow · Export notes as CSV · open · no gates yet                                 │
│  delete-notes · red · Delete a note · open · no gates yet                                        │
│Build (0)                                                                                         │
│Release (0)                                                                                       │
│Done (0) · d to show                                                                              │
│↑↓ move · Enter details · d done · t time · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Cada línea es un Change: su id, su color, su título, su estado y las puertas que ha pasado. Las
fases:

| Fase | Los Changes que están |
|---|---|
| Discover | `open`: creados, todavía sin lanzar |
| Build | `prepared`, `building`, `polishing` o `parked` |
| Release | `in-pre`, mostrado como `waiting for your G4` |
| Done | `closed`, `rejected` o `rolled-back`; plegados hasta que pulsas `d` |

Un proyecto que aún no tiene ningún Change lo dice, y dice cómo empezar uno: pídeselo a tu
agente.

```text
┌ gdd · Board · shop ──────────────────────────────────────────────────────────────────────────────┐
│No changes yet · ask your agent: /gdd-change <what you want> ($gdd-change in Codex)               │
│↑↓ move · Enter details · d done · t time · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` abre los detalles del Change seleccionado: su tipo, su color, su estado, sus puertas,
sus rutas planificadas, la ruta de su Decision Inbox y en qué se fue su tiempo.

```text
┌ gdd · Board · notes ─────────────────────────────────────────────────────────────────────────────┐
│delete-notes · Delete a note                                                                      │
│Type: Feature                                                                                     │
│Color: red                                                                                        │
│State: open                                                                                       │
│Gates: no gates yet                                                                               │
│Paths: src/notes/**, tests/**                                                                     │
│Inbox: .gdd/changes/delete-notes/inbox.md                                                         │
│Time: nothing measured yet                                                                        │
│Esc to go back · q quit                                                                           │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`t` abre el tiempo del proyecto: el tiempo de flujo de cada Change y cuánto de él fue activo
(agentes, puertas, release), en espera y entre pasos.
[Mide el trabajo](../how-to/measure.md#2-mira-en-qué-se-va-el-tiempo) explica cada número.

```text
┌ gdd · Board · notes ─────────────────────────────────────────────────────────────────────────────┐
│Where the time goes                                                                               │
│Project: 2 change(s) · flow time 0s · active 0s (0%) · waiting 0s · between steps 0s              │
│export-csv · Export notes as CSV · in progress                                                    │
│  Flow time 0s · active 0s (0%) · waiting 0s · between steps 0s                                   │
│delete-notes · Delete a note · in progress                                                        │
│  Flow time 0s · active 0s (0%) · waiting 0s · between steps 0s                                   │
│↑↓ scroll · Esc to go back · q quit                                                               │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Teclas

| Tecla | En la lista | En los detalles o el tiempo |
|---|---|---|
| `↑` `↓` | mueve la selección | desplaza el contenido |
| `Enter` | abre los detalles del Change seleccionado | |
| `t` | abre el tiempo del proyecto | |
| `d` | muestra o pliega Done | |
| `Esc` | | vuelve a la lista |
| `q`, `Ctrl-C` | cierra el Board | cierra el Board |

Sin terminal:

```text
error: gdd board needs a terminal; gdd status lists the same changes
```

## La sentada

[`gdd sitting CHANGE`](cli/sitting.md) abre, en una sola página, todo lo que un Change necesita
de ti antes del build: los escenarios tal como los tiene la rama del Change, las zonas rojas
cuando esperan aprobación y el plan de un Change rojo. La skill `gdd-change` te muestra lo mismo
en el chat; la pantalla registra las mismas decisiones.

```bash
gdd sitting export-csv
```

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│export-csv · Export notes as CSV · yellow                                                         │
│Scenarios                                                                                         │
│features/export_csv.feature                                                                       │
│Feature: Export notes as CSV                                                                      │
│  So that I can open my notes in a spreadsheet, notes writes them as CSV: a header line,          │
│  then one line per note, in the order they were written.                                         │
│  Scenario: Each note becomes one line                                                            │
│    Given the notes "Buy milk" and "Call the bank"                                                │
│    When I export the notes as CSV                                                                │
│    Then the CSV is:                                                                              │
│      """                                                                                         │
│      note                                                                                        │
│      Buy milk                                                                                    │
│      Call the bank                                                                               │
│      """                                                                                         │
│features/hidden/export_csv.feature · hidden: the Builders never see it                            │
│Feature: Export notes as CSV (hidden examples)                                                    │
│  Scenario: Other notes, same rule                                                                │
│    Given the notes "Water the plants" and "Pay the rent"                                         │
│    When I export the notes as CSV                                                                │
│    Then the CSV is:                                                                              │
│      """                                                                                         │
│      note                                                                                        │
│↑↓ scroll · a approve · c ask for changes · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

La página se desplaza con `↑` `↓`. Debajo de los escenarios vienen los ejemplos ocultos,
marcados `hidden: the Builders never see it`, y después las zonas rojas y el plan:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
...
│features/hidden/export_csv.feature · hidden: the Builders never see it                            │
│Feature: Export notes as CSV (hidden examples)                                                    │
│  Scenario: Other notes, same rule                                                                │
│    Given the notes "Water the plants" and "Pay the rent"                                         │
│    When I export the notes as CSV                                                                │
│    Then the CSV is:                                                                              │
│      """                                                                                         │
│      note                                                                                        │
│      Water the plants                                                                            │
│      Pay the rent                                                                                │
│      """                                                                                         │
│Red zones to approve                                                                              │
│gdd/**, .github/**, .claude/**, .agents/**, .opencode/**, .gdd/bin/** · The harness itself:       │
│touching the gates is touching the safety of the method                                           │
│pyproject.toml, uv.lock, osv-scanner.toml, osv-scanner-licenses.toml · Dependencies and toolchain:│
│supply chain                                                                                      │
│Plan: not needed for a yellow change                                                              │
│↑↓ scroll · a approve · c ask for changes · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Un Change rojo muestra al final su plan, sacado de `.gdd/changes/CHANGE/plan.md`. Un Change sin
escenarios dice `No scenario files on change/CHANGE`, y las zonas rojas ya aprobadas dicen
`Red zones: approved`:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│delete-notes · Delete a note · red                                                                │
│Scenarios                                                                                         │
│No scenario files on change/delete-notes                                                          │
│Red zones: approved                                                                               │
│Plan                                                                                              │
│# Plan · delete-notes                                                                             │
│What: `notes delete N` removes note N and keeps the others in order.                              │
│How: a function `delete(notes, n)` in src/notes/store.py, called by the command line.             │
│Risks: deleting the wrong note loses data. The command asks before it deletes.                    │
│Roll back: revert the merge commit; the notes file format does not change.                        │
│↑↓ scroll · a approve · c ask for changes · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Cuando los escenarios ya están aprobados y nada más te espera, la sentada lo dice y no ofrece
ninguna aprobación: solo funcionan `↑` `↓` y `q`.

```text
┌ gdd · Sitting · shop ────────────────────────────────────────────────────────────────────────────┐
│export-csv · Export the catalog as CSV · yellow                                                   │
│Nothing waits for you in this sitting: G1 ok on export-csv.                                       │
│Scenarios                                                                                         │
│features/export-csv.feature                                                                       │
│Feature: Export the catalog as CSV                                                                │
│  Scenario: Every product becomes one line                                                        │
│    Given a catalog with 2 products                                                               │
│    When I export the catalog                                                                     │
│    Then the file has 3 lines                                                                     │
│Red zones: approved                                                                               │
│Plan: not needed for a yellow change                                                              │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Aprobar

`a` muestra exactamente lo que va a registrar, en tu nombre. Todavía no se escribe nada:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│This records, as you:                                                                             │
│- G1 ok on export-csv, sealing features/export_csv.feature, features/hidden/export_csv.feature    │
│- red zones ok: 2 red zone(s)                                                                     │
│y to record · n to go back                                                                        │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`y` lo registra, y la página dice qué se ha registrado y qué viene después:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│Recorded                                                                                          │
│G1 ok on export-csv (human).                                                                      │
│Red zones ok (2 red zone(s)).                                                                     │
│Next: tell your agent it is approved (it runs gdd launch export-csv)                              │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Lo que mostró la sentada | Lo que registra `y` | Dónde |
|---|---|---|
| Escenarios | G1 OK, que sella cada fichero de escenarios: gdd guarda su hash, así que ningún agente puede cambiarlo sin una nueva aprobación | el `change.json` del Change (`gates.G1`) y `.gdd/log.jsonl` |
| Zonas rojas por aprobar | la aprobación de las zonas rojas | `.gdd/zones.json` y el log |
| Un plan | la aprobación del plan | `change.json` (`gates.plan`) y el log |

Son los mismos registros que [`gdd gate`](cli/gate.md) con `--by human --note sitting`. G1 se
registra desde un worktree desechable de la rama del Change, así que tu checkout se queda en
`main`. Para el Change rojo de antes, `a` e `y` registran solo su plan:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│Recorded                                                                                          │
│plan ok on delete-notes (human).                                                                  │
│Next: tell your agent it is approved (it runs gdd launch delete-notes)                            │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Si los escenarios, las zonas rojas o el plan cambian después de que abras la página, `y` no
registra nada: la página muestra lo que tiene ahora el proyecto, bajo
`This changed after you read it: read it again before approving`.

### Pedir cambios

`c` te pide tus palabras:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│What should change?                                                                               │
│> _                                                                                               │
│Enter to continue · Backspace erases · Esc to go back                                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` muestra lo que va a registrar, e `y` lo registra: G1 KO sobre el Change, con tus
palabras como nota, en el registro del Change y en el log.

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│G1 ko on export-csv: Add an example with an empty list                                            │
│y to record · n to go back                                                                        │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Teclas

| Tecla | Qué hace |
|---|---|
| `↑` `↓` | desplaza la página |
| `a` | aprobar: muestra lo que va a registrar |
| `c` | pedir cambios: te pide tus palabras |
| `y` | en una confirmación, la registra |
| `n`, `Esc` | en una confirmación o en la línea de texto, vuelve atrás; no se registra nada |
| `Enter` | en la línea de texto, pasa a la confirmación, cuando ya has escrito algo |
| `Backspace` | en la línea de texto, borra |
| `q`, `Ctrl-C` | cierra la sentada |

Una vez lanzado un Change, su sentada ha terminado: la página dice
`CHANGE is STATE: its sitting is over` y solo se desplaza. Un Change que te espera en G4 abre en
su lugar la página de G4.

Sin terminal, el comando dice cómo registrar las mismas decisiones:

```text
error: gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok export-csv --by human (and gdd gate zones ok --by human, gdd gate plan ok export-csv --by human for a red change)
```

## La sentada de G4

Cuando un Change espera en la puerta de salida, `in-pre`, `gdd sitting CHANGE` muestra lo que
necesitas para juzgarlo, en este orden:

- cómo funciona, en palabras del Reviewer;
- el veredicto del Reviewer;
- la demo;
- lo que los agentes decidieron sin ti;
- la línea de changelog;
- en qué se fue el tiempo del Change.

<!-- Real screen of gdd 0.1.0 at 100 columns: blank lines and part of the Reviewer's words and of
     the notes trimmed. -->
```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│export-json · Export notes as JSON · waiting for your G4                                          │
│How it works                                                                                      │
│- `export_json` sits beside CSV export in `src/notes/export.py`.                                  │
│- Python’s JSON serializer preserves note order and escapes string contents.                      │
│- Acceptance steps follow the existing fixture pattern.                                           │
│- Unit tests cover empty lists, ordering, and embedded quotes.                                    │
│Reviewer: OK                                                                                      │
│No blocking findings. The change meets the package scope, reuses the existing Given step, and     │
│makes real assertions. Recorded G2 and G3 results are green; tests were not rerun during this     │
...
│Demo: none, the project has no demo command                                                       │
│Decided without you                                                                               │
│- Reordered imports in tests/acceptance/steps/export_csv.py (ruff I001) so lint passes in G2; no  │
│behaviour change, same steps folder as the package paths.                                         │
...
│Changelog: none set                                                                               │
│Flow time 6m · active 6m (97%) · waiting 9s · between steps 1s                                    │
│Agents: orchestrator · opus 2m, builder-build · sonnet 2m, builder-test · sonnet 1m, reviewer ·   │
│gpt-6-astra 32s                                                                                   │
│Gates: security 1s, acceptance 1s, unit 1s, secrets 1s, architecture 0s, types 0s, acceptance_full│
│0s, lint 0s, dependencies 0s, lockfile 0s, path-guard 0s                                          │
│Waiting: at the sitting 5s, to start 4s                                                           │
│↑↓ scroll · o OK · k KO · p polish · q quit                                                       │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Demo: none, the project has no demo command` significa que `demo` en `[release]` de
`gdd/commands.toml` está vacío; `How it works: the Reviewer did not explain the change` y
`Reviewer: no review yet` ocupan su sitio cuando a la revisión le faltan. Las notas de los
agentes bajo *Needs your decision* y *Parked* no están en esta página: léelas en la Decision
Inbox, [`gdd inbox CHANGE`](cli/inbox.md).

### Responder

Cada respuesta pasa por una confirmación que muestra el comando exacto que ejecuta, en tu
nombre, y lo que viene después. No se ejecuta nada antes de `y`.

| Tecla | Respuesta | Pregunta antes | Después la confirmación dice |
|---|---|---|---|
| `o` | OK | nada | `gdd resume CHANGE --g4 ok` · The change is promoted. |
| `k` | KO | `What should change?` | `gdd resume CHANGE --g4 ko --note "WORDS"` · The change is rejected and stays out of production. |
| `p` | polish | `What should change?` | `gdd resume CHANGE --g4 polish --note "WORDS"` · One more Builder round with your words, then the gates and the demo again. The agents run now: it takes minutes. |

`y` en la confirmación cierra la pantalla y ejecuta ese [`gdd resume`](cli/resume.md) en la
terminal, para que veas su progreso. `n` o `Esc` vuelve atrás y no ejecuta nada. Si alguien
respondió al Change mientras tanto, `y` dice `CHANGE is STATE now: nothing was run`.
[Responder a la puerta de salida](../how-to/release-gate.md) muestra la confirmación y la línea
de texto.

Un Spike no tiene Reviewer, demo ni línea de changelog: su página lo dice, y OK lleva a `main`
solo sus ficheros de ADR de `docs/adr/`. En un Spike con variantes, `o` y `p` preguntan primero
`Which variant? Its number:`. Consulta [Explorar con un Spike](../how-to/spikes.md).

### Teclas

| Tecla | Qué hace |
|---|---|
| `↑` `↓` | desplaza el contenido |
| `o` | OK: promueve el Change |
| `k` | KO: lo rechaza, con tus palabras |
| `p` | polish: una ronda más del Builder, con tus palabras |
| `y` | en la confirmación, cierra la pantalla y ejecuta la respuesta |
| `n`, `Esc` | en la confirmación o en la línea de texto, vuelve atrás; no se ejecuta nada |
| `q`, `Ctrl-C` | cierra la sentada; no se ejecuta nada |

## Ver también

- [Aprobar los escenarios](../how-to/write-scenarios.md): qué mirar en la sentada.
- [Responder a la puerta de salida](../how-to/release-gate.md): qué comprobar antes de decir OK.
- [La skill gdd-change](skills/change.md): la misma sentada y G4, en el chat.
- [Ficheros](files.md): `change.json`, `zones.json` y el log que escribe la sentada.
