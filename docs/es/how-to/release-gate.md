# Responder a la puerta de salida

Decide si un Change que espera en la puerta de salida (G4, *Release Gate*) pasa a producción:
mira lo que hace y responde OK, KO o polish.

## Antes de empezar

- Un Change en pausa en G4. `bin/gdd status` lo muestra como `in-pre`, el Board lo pone bajo
  Release como `waiting for your G4`, y `gdd launch` terminó con `Paused at G4`.
  [Ejecutar un Change](run-a-change.md) te lleva hasta ahí.

## Cuándo te espera un Change

G4 es la única parada después de la construcción, y solo algunos Changes pasan por ella. Un
Change espera a una persona en G4 cuando:

- tiene G1, porque crea o modifica un escenario (`g4_if_g1 = true` en la vía completa de su
  color en `gdd/rules.toml`, el valor por defecto); o
- es rojo: un Change rojo siempre se encuentra con una persona en G4, y gdd rechaza un fichero
  de reglas que lo desactive.

A cualquier otro Change lo promueven las máquinas en cuanto pre está en verde. El campo `g4` de
`bin/gdd classify CHANGE` dice en qué caso está un Change. Un Spike también se para en G4,
donde eliges lo que dice su ADR: consulta [Explorar con un Spike](spikes.md).

Cuando un Change llega a esperarte, ya ha pasado mucho sin ti. Ha superado la puerta del bucle
(G2, *Loop Gate*) y la puerta de integración (G3, *Merge Gate*), el Reviewer ha dicho OK, gdd
lo ha fusionado en `main` con su registro en el commit de fusión, lo ha construido una vez y ha
desplegado esa build en pre. Después, pre ha ejecutado sus pruebas y la demo: vídeo, capturas
de pantalla o, en una biblioteca o una herramienta sin pantalla, la ejecución legible de sus
escenarios, que queda en `.gdd/changes/CHANGE/demo/`. Lo que apruebas es esa build, tal como
funciona en pre.

`gdd init` deja vacíos todos los pasos de la fase Release, con `simulated = true`, en la tabla
`[release]` de `gdd/commands.toml`: hasta que rellenes `deploy_pre`, `demo` y los demás, pre y
producción son simulados y la sentada dice `Demo: none, the project has no demo command`.

## 1. Leer la Decision Inbox

La Decision Inbox es el único mensaje que gdd mantiene por cada Change. En G4 empieza
explicando cómo funciona el Change:

```bash
bin/gdd inbox export-json
```

```text
# Decision Inbox · export-json · Export notes as JSON

Color yellow · lane full · state in-pre

## How it works
- `export_json` sits beside CSV export in `src/notes/export.py`.
- Python’s JSON serializer preserves note order and escapes string contents.
- Acceptance steps follow the existing fixture pattern.
- Unit tests cover empty lists, ordering, and embedded quotes.

## Needs your decision
- Lint I001 (import order) remains in tests/acceptance/steps/export_csv.py, left over from export-csv; not touched here. The 'the notes ... and ...' Given step is reused from export_csv.py.
...
- G4 · Release Gate: check the pre demo and the change message, then answer with gdd resume export-json --g4 ok (promote to production), --g4 ko (reject: production untouched) or --g4 polish --note <what to polish> (one more Builder round).

## Decided without you
- Reordered imports in tests/acceptance/steps/export_csv.py (ruff I001) so lint passes in G2; no behaviour change, same steps folder as the package paths.
...

## Parked
- Learn: export-csv closed with G2 lint green, but its unchanged tests/acceptance/steps/export_csv.py now fails ruff I001 at the same ruff version. Whether ruff treats 'notes' as first-party seems to depend on the environment. Consider pinning [tool.ruff] src = ["src"] or known-first-party = ["notes"] so lint gives the same result in every worktree. Not blocking export-json: the replan fixes the import order in a test round.
```

- **How it works** (cómo funciona) es la explicación breve del Reviewer sobre el código que
  tocó el Change: dónde está, cómo encaja, qué patrón sigue. Te permite aprobar lo que
  entiendes sin leer cada línea. Cuando el Reviewer no la escribió, la bandeja lo dice.
- **Needs your decision** (necesita tu decisión) recoge la petición de G4 y las notas que los
  agentes te dejaron por el camino. Aquí el lint de G2 falló por el orden de los imports de un
  fichero de pasos de un Change anterior, que el Builder en modo build no puede editar, hasta
  que el Orchestrator replanificó y el Builder en modo test lo arregló. Las notas cuentan esa
  historia; léelas para saber por lo que pasó el Change.
- **Decided without you** (decidido sin ti) enumera lo que los agentes eligieron por su cuenta
  y las rutas fuera del plan que aceptó la comprobación de alcance.
- **Parked** (aparcado) enumera lo que un agente dejó para más adelante. Aquí es una lección
  sobre la configuración del lint, que no detiene el Change.

Cuando corresponde, siguen más secciones: las excepciones de seguridad que añade el Change, la
deuda conocida que sigue fallando o que ya se ha pagado, y tus notas de polish anteriores. La
bandeja también se guarda en `.gdd/changes/CHANGE/inbox.md`.

## 2. Abrir la sentada de G4

La sentada muestra, en una sola pantalla, casi todo lo que necesitas para juzgar el Change:

```bash
bin/gdd sitting export-json
```

<!-- Pantalla real de gdd 0.1.0 a 100 columnas: recortadas las líneas en blanco y parte de las
     palabras del Reviewer y de las notas. -->
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

De arriba abajo: cómo funciona el Change, el veredicto del Reviewer y sus palabras, la demo
(con sus cifras cuando el comando de demo las escribió, por ejemplo
`Demo: 12 scenarios (12 passed)`), lo que los agentes decidieron sin ti, la línea de changelog
fijada con [`gdd changelog-line`](../reference/cli.md#gdd-changelog-line) y en qué se fue el
tiempo del Change: aquí, casi todo se fue en tres rondas del Builder por el fallo de lint y en
la replanificación del Orchestrator. `Changelog: none set` significa que nadie la fijó:
entonces el commit de fusión lleva la línea por defecto, el título bajo la sección del tipo del
Change (`Added` para una Feature).

Las notas de los agentes en *Needs your decision* y *Parked* no aparecen en la sentada: léelas
antes en la bandeja.

| Tecla | Qué hace |
|---|---|
| `↑` `↓` | desplaza el contenido |
| `o` | OK: promueve el Change |
| `k` | KO: lo rechaza, con tus palabras |
| `p` | polish: una ronda más del Builder, con tus palabras |
| `q` | cierra la sentada; no se ejecuta nada |

Cada respuesta pasa por una confirmación que muestra el comando exacto y lo que viene después.
Al pulsar `o` aparece:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│This runs, as you:                                                                                │
│gdd resume export-json --g4 ok                                                                    │
│The change is promoted.                                                                           │
│y to run · n to go back                                                                           │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`y` cierra la pantalla y ejecuta ese comando en el terminal, para que veas su progreso; `n`
vuelve atrás y no se ejecuta nada. `k` y `p` primero te piden tus palabras, porque las
necesitan:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│What should change?                                                                               │
│> _                                                                                               │
│Enter to continue · Backspace erases · Esc to go back                                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Después, KO dice "The change is rejected and stays out of production." y polish dice "One more
Builder round with your words, then the gates and the demo again." Si alguien ha respondido al
mismo Change mientras tanto, la sentada lo dice y no ejecuta nada.

## 3. Comprobar antes de decir OK

G4 es donde *terminado* se encuentra con *bueno*. Todas las puertas han pasado, así que la
pregunta ya no es si funciona, sino si es lo que quieres. Mira:

1. **La demo en pre.** ¿Hace lo que prometían los escenarios, en la interfaz real?
2. **How it works.** ¿Coincide la explicación con lo que pediste y con la forma que quieres que
   conserve el código?
3. **Las palabras del Reviewer.** Un OK puede traer observaciones que merecen un polish.
4. **Decided without you.** Cada decisión es una que podrías haber tomado de otra manera.
5. **Las excepciones de seguridad y la deuda conocida**, cuando la bandeja las enumera.

Si algo no está bien pero le falta poco, mejor polish que KO: conserva el trabajo.

## 4. Responder

Responde desde el checkout principal, en la sentada o con un comando. Solo puede hacerlo una
persona: si un Builder o un Reviewer ejecuta `gdd resume`, se rechaza.

### OK: promoverlo

```bash
bin/gdd resume export-json --g4 ok
```

```text
Released v0.2.0
Change export-json closed. Message: .gdd/changes/export-json/inbox.md · Tokens: gdd report --change export-json
```

gdd registra tu OK, promueve a producción **la misma build** que funcionó en pre, sin volver a
construirla, y después observa producción. Cuando la observación pasa, el Change se cierra y su
worktree desaparece; su rama se queda.

- **Versiones.** Con `[versions] mode = "auto"` (el valor por defecto), un Change promovido que
  tiene línea de changelog recibe el siguiente tag de versión en cuanto pasa la promoción: aquí
  `v0.2.0`, porque el proyecto estaba en `v0.1.0` y una Feature va en `Added`, que sube la
  versión menor. Consulta
  [Publicar versiones y llevar un changelog](releases-and-changelog.md).
- **Reversión (*rollback*).** Cuando la observación falla, gdd ejecuta el paso `rollback` para
  volver a la versión anterior, el Change termina en `rolled-back` y su bandeja pide un Hotfix:
  "Production failed while observing: rolled back to the previous version. A hotfix is needed."
- **Una promoción fallida** deja producción intacta y el Change sigue en pausa en G4, con el
  motivo en su bandeja. Arregla la causa y vuelve a responder OK: el mismo OK la reintenta.
- **Una promoción interrumpida** (la máquina se paró a mitad) aparca el Change en vez de
  adivinar, con un mensaje que te pide mirar primero producción, para que nada se despliegue
  dos veces.

### KO: rechazarlo

```bash
bin/gdd resume export-json --g4 ko --note "Not what I asked"
```

gdd registra tu KO y tus palabras, deja producción intacta y cierra el worktree; el Change
termina en `rejected`. KO no toca `main`: el commit de fusión de G3 se queda ahí.

### Polish: una ronda más

```bash
bin/gdd resume export-json --g4 polish --note "Indent the JSON, one note per line"
```

Polish necesita tus notas; sin `--note`, gdd se niega:

```text
error: polish needs your notes: gdd resume export-json --g4 polish --note ...
```

El Builder recibe una ronda más en modo build con tus notas. Después, el Change vuelve a pasar
por G2, G3, el Reviewer, la fusión cuando hay algo nuevo y pre, y se para en G4 con
`Paused at G4`. Respóndelo otra vez como antes. Polish sustituye a un KO, no añade una parada,
y tus notas se quedan en la bandeja bajo Polish. [`gdd learn`](../reference/cli.md#gdd-learn)
enumera las notas que se repiten entre Changes, para que se conviertan en una línea de
`docs/standards.md` o en una comprobación.

Las palabras de `--note` son el resto de la línea, así que las comillas son opcionales. Si el
Change no está esperando, gdd lo dice:

```text
error: Change export-json is not paused at G4.
```

En un Spike con variantes, `--variant N` indica qué variante lleva OK a `main` o cuál vuelve a
construir polish.

## Siguientes pasos

- [Volver a poner en marcha un Change aparcado](parked-change.md): cuando un Change se para
  antes de G4.
- [Publicar versiones y llevar un changelog](releases-and-changelog.md): tags, `gdd release` y
  `CHANGELOG.md`.
- [Pocas paradas para la persona](../explanation/human-stops.md): por qué G4 es la única parada
  después de la construcción y por qué terminado no significa bueno.
