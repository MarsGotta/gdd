# Puertas

Qué comprueba cada puerta de GDD, cuándo se ejecuta, quién decide, sus límites y qué aparca un
Change, por color. Los valores son los del `gdd/rules.toml` que escribe `gdd init`; consulta
[Configuración](configuration.md#rulestoml) para cada clave.

## Puertas por color

El color de un Change sale de las rutas que planifica y toca (consulta
[zones.toml](configuration.md#zonestoml)); su vía, de su color, de sus escenarios y de si es
reanudable (consulta [Vías](#vías)). Lee cada columna como un tipo de Change.

| | Verde, rápida | Verde, completa | Amarillo, rápida | Amarillo, completa | Rojo |
|---|---|---|---|---|---|
| G0, puerta de preparación (una vez por proyecto) | antes de cualquier Change | antes de cualquier Change | antes de cualquier Change | antes de cualquier Change | antes de cualquier Change |
| Zonas rojas aprobadas | antes de cualquier Change salvo un Spike | igual | igual | igual | igual |
| G1, puerta de escenarios | nunca: sin escenarios nuevos | cuando hay escenarios nuevos o cambiados | nunca | cuando hay escenarios nuevos o cambiados | cuando hay escenarios nuevos o cambiados |
| Plan aprobado por la persona | no | no | no | no | siempre |
| Modo test antes del modo build | no | solo con G1 | sí | sí | sí |
| G2, puerta del bucle | cada ronda | cada ronda | cada ronda | cada ronda | cada ronda |
| G3, puerta de integración | sí | sí | sí | sí | sí |
| Mutation testing (pruebas de mutación) en G2 y G3 | no | no | no (`mutation = true` lo activa) | no (`mutation = true` lo activa) | siempre |
| Reviewer | no | quick | quick | quick | deep, seguridad incluida |
| G4, puerta de salida, a cargo de la persona | no | solo con G1 | no | solo con G1 | siempre |
| Paradas humanas | 0 | de 0 a 2 | 0 | de 0 a 2 | 2: la sentada y después G4 |

El rojo nunca va por la vía rápida. Un Spike sigue una receta propia: consulta
[Spikes](#spikes).

El núcleo de GDD siempre está activo para el rojo: mutation testing, el Reviewer (deep) y el G4
humano. Se rechaza un fichero de reglas que desactive cualquiera de ellos para el rojo.

## Vías

| Vía | Cuándo | Qué cambia |
|---|---|---|
| Vía rápida (*Fast Lane*) | El color no es rojo, el Change no crea ni cambia ningún `.feature` que necesite G1, y se creó con `gdd new-change --resumable` (otro agente podría retomarlo solo con su ticket). | Su regla es `[color.<color>.fast]`. Con las tablas de `gdd init`: sin G1, sin G4 humano, cero paradas humanas; el verde además va sin modo test y sin Reviewer. |
| Vía completa (*Full Lane*) | Cualquier otro Change. | Su regla es `[color.<color>.full]`. |

El color manda sobre el tipo de trabajo y la vía: un Change rojo siempre toma la vía completa y
el núcleo de GDD. El Orchestrator juzga si un Change es reanudable cuando lo crea;
`gdd classify` imprime la vía y el motivo.

## G0 Setup Gate

**Cuándo:** Una vez por proyecto, al final de Setup (fase 0), antes de que se pueda preparar
ningún Change.

**Qué se ejecuta:** `gdd selftest`, el autotest del arnés, con cuatro comprobaciones:

1. Un test que falla pone G2 en rojo: con el fichero de `[selftest] file` apartado, cada
   comando de G2 pasa, o queda sujeto a su deuda conocida de `gdd/debt.toml` igual que en G2;
   con el test que falla de `[selftest] content` colocado ahí, uno falla. Un `gdd/debt.toml`
   que gdd no puede leer hace fallar la comprobación, con el motivo.
2. Editar un fichero protegido está bloqueado: `[protected] all` cubre `gdd/rules.toml` y
   `.github/workflows/gdd.yml`, y ese workflow ejecuta `bin/gdd ci`.
3. El rollback funciona en pre: `[release] rollback_pre` pasa, o el release es simulado.
4. Cero escenarios cuentan como fallo: G3 no cuenta ninguno en una carpeta vacía, y
   `[acceptance] required` no es `false`.

El resultado va a `.gdd/selftest.json` y al log. `gdd selftest` sale con 1 salvo que pasen las
cuatro (`harness 4/4`).

**Quién decide:** Las máquinas: `gdd gate G0 ok --by machines`. Solo se abre cuando existe
`docs/brief.md` y el último autotest pasó; una persona no puede abrirla (se rechaza
`--by human`).

**Límites:** Ninguno. Arregla el arnés y vuelve a ejecutar `gdd selftest`.

**Qué bloquea:** `gdd prepare` rechaza cualquier Change mientras G0 esté pendiente.

En G0 no hay más mapa de zonas que el que escribió `gdd init`: el arnés y la cadena de
suministro son rojos, y el código sin mapear es amarillo.

### Cuándo dice que no

Mensajes reales, primero los más frecuentes.

Ejecutas `gdd prepare` antes de que G0 esté abierta:

```text
error: G0 pending: run the Setup (the gdd-setup command).
```

Main ya falla un comando de G2 (aquí lint, por un import sin usar), así que el autotest no
puede ver que un test que falla pone G2 en rojo. Arréglalo o regístralo como deuda conocida
con `gdd adopt`:

```text
KO a failing test turns G2 red (G2 is red without it: lint)
OK editing a protected file is blocked (Path Guard and CI)
OK rollback works in pre
OK zero scenarios counts as a failure
harness 3/4
```

Intentas abrir G0 cuando el último autotest no es 4/4:

```text
error: G0 cannot open: the harness self-test is not 4/4 (bin/gdd selftest).
```

Falta el Brief:

```text
error: G0 cannot open: docs/brief.md (Project Brief) is missing.
```

## G1 Scenario Gate

**Cuándo:** En la sentada de un Change, cuando el Change crea o cambia un fichero `.feature`
dentro de `[acceptance] dir` y fuera de `[acceptance] regression`. Cambiar solo escenarios de
regresión no necesita G1.

**Qué se ejecuta:** Nada: la persona lee los escenarios (`gdd sitting <id>` los muestra) y los
aprueba o pide cambios. Aprobar G1 sella en el registro los ficheros de escenarios del Change
tal como los tiene su rama, con su SHA-256, ejemplos ocultos incluidos.

**Quién decide:** La persona: `gdd gate G1 ok <id> --note "..."`, o `gdd gate G1 ko <id>` para
pedir cambios.

**Límites:** G1 rechaza los ejemplos ocultos (`<dir>/hidden/`) cuyas frases de paso no aparecen
en ningún escenario visible: el modo test nunca los ve y no podría escribir sus pasos. Una sola
sentada por Change cubre G1, las zonas rojas y el plan.

**Qué bloquea:** `gdd prepare` rechaza un Change con escenarios nuevos o cambiados sin G1. Tras
G1, cualquier cambio en un fichero sellado hace fallar Path Guard para todos los roles
(`scenario approved in G1 and modified afterwards`).

### Zonas rojas

Las zonas rojas de `gdd/zones.toml` necesitan una aprobación humana, una vez y de nuevo cada
vez que cambia cualquier zona roja: `gdd gate zones ok --note "..."` (solo `--by human`). La
aprobación guarda las zonas rojas y su hash en `.gdd/zones.json`; aprobarlas tal como ya están
no escribe nada. Hasta que se aprueban, solo se puede preparar un Spike. `gdd status` dice
`pending` o `changed`.

### El plan de un Change rojo

Un Change rojo necesita que la persona apruebe su plan antes de prepararse: el Orchestrator
escribe `.gdd/changes/<id>/plan.md` (qué, cómo, riesgos, cómo revertir), la persona lo lee en la
misma sentada y `gdd gate plan ok <id>` lo registra.

### Cuándo dice que no

Mensajes reales, primero los más frecuentes.

El Change tiene escenarios nuevos que nadie ha aprobado todavía, así que `gdd prepare` lo
rechaza:

```text
error: New or changed scenarios without G1: a human must approve them (the gdd-change command).
```

Lees los escenarios y pides cambios (`gdd gate G1 ko cart-total`):

```text
G1 ko on cart-total (human).
```

Las zonas rojas del proyecto aún no están aprobadas, normalmente en su primer Change:

```text
error: Red zones not approved yet: the Orchestrator proposes them at the end of Discover. (bin/gdd gate zones ok --by human)
```

Un ejemplo oculto usa una frase que no tiene ningún escenario visible:

```text
error: G1 refused: hidden examples use step phrasings that no visible scenario has, so test mode can never write them. Reuse visible sentences with new data:
  - an empty cart
```

## G2 Loop Gate

**Cuándo:** Después de cada ronda del modo build; cada vez que un Builder ejecuta `bin/gdd g2`
(el modo test lo ejecuta para ver que sus tests nuevos fallan por el motivo correcto); y en un
relanzamiento, antes de que se ejecute ningún Builder.

**Qué se ejecuta, en orden:**

1. Path Guard para el rol, sobre lo que el Change ha tocado desde que empezó su build. Si
   falla, no se ejecuta ningún comando: un rol que haya tocado la configuración de la puerta
   podría hacer suyos los comandos.
2. Cada comando de `[g2]` de `gdd/commands.toml`, en el orden alfabético de sus nombres, cada
   uno dentro de `[limits] command_seconds`. Los comandos `mutation*` solo se ejecutan cuando
   el Change necesita mutation testing (rojo, o una regla con `mutation = true`).
3. Path Guard otra vez.
4. En modo build, la comprobación de alcance: una ruta fuera de las rutas planificadas cuyo
   color es el del Change o inferior se acepta y se lista en la Decision Inbox; una de color
   superior falla.

El informe va a `.gdd/changes/<id>/g2.json` y al log (una línea por comando, una para Path
Guard y una para la puerta), salvo que lo haya ejecutado un agente.

**Quién decide:** Las máquinas. Pasa cuando nada ha fallado. Un comando con deuda conocida en
`gdd/debt.toml` que falla solo avisa, salvo que informe de más que su recuento.

**Límites:** `[limits] builder_retries` rondas del modo build (3), al menos una. Si sigue en
rojo: `[limits] replans` replanificaciones (1), cada una con una llamada al Orchestrator que
escribe `replan.md`, una ronda del modo test cuando la replanificación termina con
`NEXT: test`, una ronda más del modo build y G2. Solo el Orchestrator amplía el alcance.

**Qué aparca:** G2 todavía en rojo tras las rondas y las replanificaciones
(`G2 still red after 3 retries and 1 replan`). Path Guard falla tras una ronda del modo test
(`Path Guard failed after test mode: ...`).

Un relanzamiento cuyo código ya pasa G2 se salta el Builder (`resume_green` en la decisión).

### Cuándo dice que no

Mensajes reales, primero los más frecuentes.

Falla un comando de G2, aquí lint por un import sin usar. `gdd g2` imprime su informe en
JSON:

```text
{"ok":false,"gate":"G2","color":"yellow","passed":["acceptance","architecture","types","unit","path-guard","scope"],"skipped":[],"failures":[{"name":"lint","ok":false,"code":1,"duration_s":0.0,"tail":"F401 [*] `os` imported but unused\n --> src/shop/cart.py:1:8\n ...\nFound 1 error.\n[*] 1 fixable with `--fix`.\n"}],"out_of_scope":[]}
```

El Builder agotó sus rondas y la replanificación, y G2 sigue en rojo. El Change se aparca;
su Decision Inbox dice:

```text
## Parked
- G2 still red after 3 retries and 1 replan
```

El Builder en modo build edita un escenario aprobado en G1. Path Guard falla y no se
ejecuta ningún comando:

```text
{"ok":false,"gate":"G2","color":"yellow","passed":["scope"],"skipped":[],"failures":[{"name":"path-guard","tail":"features/cart_total.feature: protected path for builder-build\nfeatures/cart_total.feature: scenario approved in G1 and modified afterwards\nThe gate commands did not run: Path Guard must pass first."}],"out_of_scope":[]}
```

El Builder toca una ruta roja que el plan no incluye:

```text
{"ok":false,"gate":"G2","color":"red","passed":["acceptance","architecture","lint","types","unit","path-guard"],"skipped":[],"failures":[{"name":"scope","tail":"src/shop/payments/charge.py: outside the package and red (the change is yellow); remove it, or the Orchestrator widens the scope"}],"out_of_scope":[]}
```

## G3 Merge Gate

**Cuándo:** En cuanto G2 está en verde, y otra vez sobre el último main cuando main ha avanzado
mientras se construía el Change.

**Qué se ejecuta, en orden:**

1. Path Guard, después cada comando de `[g3]` (en orden alfabético, dentro de
   `command_seconds`, `mutation*` solo cuando hace falta), después Path Guard otra vez, y la
   comprobación de alcance en modo build, como en G2. Las fichas de stack ponen aquí la
   ejecución completa de los escenarios (ejemplos ocultos incluidos), el fichero de lock, el
   análisis estático de seguridad, las licencias de las dependencias, los secretos en el diff y
   mutation testing sobre el diff.
2. El recuento de escenarios: cero escenarios en `[acceptance] dir` hacen fallar la puerta
   cuando `[acceptance] required` es `true` o el Change tiene ficheros de escenarios propios.
3. Las excepciones de seguridad (`[[IgnoredVulns]]` de `osv-scanner.toml`,
   `[advisories] ignore` de `deny.toml`): una sin motivo hace fallar la puerta; las que añade
   el Change se listan en la Decision Inbox y en G4.
4. Cuando el Change tiene Reviewer: la revisión, con la profundidad de su regla. El Reviewer
   responde `VERDICT: OK` o `VERDICT: KO` en la última línea de su revisión.

El informe va a `.gdd/changes/<id>/g3.json` y al log.

**Quién decide:** Las máquinas y el Reviewer. Cuando ambos pasan, gdd registra G3 `ok` por
`machines` y después fusiona.

**Límites:** G3 en rojo: una corrección del Builder (`fix=g3`), G2, y G3 otra vez, que decide.
KO del Reviewer: una corrección (`fix=review`), G2, G3 y una segunda revisión que comprueba
solo los motivos de la primera. Las fusiones van de una en una; un Change cuyo main ha avanzado
toma el último main y vuelve a pasar G3.

**Qué aparca:** G3 todavía en rojo tras su corrección (`G3 still red after one fix`). Un KO, o
un G3 en rojo, tras la corrección pedida por el Reviewer
(`Reviewer KO or G3 red after one fix`). Un conflicto con el último main
(`main moved under it and the merge has conflicts in ...`). G3 en rojo sobre el último main.

Tras la fusión, el commit de fusión lleva el registro del Change (consulta
[Ficheros](files.md#el-commit-de-fusión)), y los pasos `build` y `deploy_pre` lo llevan a pre.

### Cuándo dice que no

Mensajes reales, primero los más frecuentes.

Falla un comando de G3, aquí por una dependencia con vulnerabilidades conocidas
(`jinja2==2.10` en `uv.lock`):

```text
{"ok":false,"gate":"G3","color":"red",...,"failures":[...,{"name":"security","ok":false,"code":1,"duration_s":1.0,"tail":"...Total 1 package affected by 6 known vulnerabilities (0 Critical, 2 High, 4 Medium, 0 Low, 0 Unknown) from 1 ecosystem.\n6 vulnerabilities can be fixed.\n...| https://osv.dev/PYSEC-2019-217      | 8.6  | PyPI      | jinja2  | 2.10    | 2.10.1        | uv.lock |\n..."}],"out_of_scope":[],...}
```

El Reviewer respondió `VERDICT: KO`, el Builder lo corrigió una vez y la segunda revisión
siguió diciendo KO. El Change se aparca:

```text
## Parked
- Reviewer KO or G3 red after one fix
```

G3 sigue en rojo tras la única corrección del Builder:

```text
## Parked
- G3 still red after one fix
```

Main avanzó mientras se construía el Change, y la fusión tiene conflictos:

```text
## Parked
- main moved under it and the merge has conflicts in crates/gdd/src/lib.rs: merge main into change/gdd-upgrade in its worktree (/home/me/gdd/../gdd.worktrees/gdd-upgrade), settle the conflicts, commit, then gdd launch gdd-upgrade
```

## G4 Release Gate

**Cuándo:** En cuanto el Change está fusionado y en pre.

**Qué se ejecuta:** `tests_pre` y después `demo` (`gdd release pre`); el Change queda `in-pre`
digan lo que digan. El G4 humano se aplica cuando el Change es rojo, cuando tuvo G1 y su regla
tiene `g4_if_g1 = true`, y siempre en un Spike: la receta se pausa
(`.gdd/changes/<id>/run.json`) con la Decision Inbox y la demo para la persona. En otro caso,
las máquinas registran G4 `ok` y promueven el Change en el acto.

**Quién decide:** La persona, con `gdd resume <id> --g4 ok|ko|polish`:

| Respuesta | Qué pasa |
|---|---|
| `ok` | `promote` (el mismo artefacto, a producción) y después `observe`. El Change queda `closed`. |
| `ko` | El Change queda `rejected`; producción no se toca; su worktree se elimina. |
| `polish --note "..."` | Una pasada más del Builder con las notas de la persona (modo build, `fix=polish`, con sus rondas y su replanificación), después G3, el Reviewer, la fusión cuando hay algo nuevo, pre y G4 otra vez. Sustituye a un KO; no es una parada más. |

**Límites:** Una promoción que falla deja producción intacta y el Change esperando en G4: el
mismo `ok` la reintenta. Con `[versions] mode = "auto"`, un Change promovido con una línea de
changelog recibe su versión en cuanto la promoción ha pasado.

**Qué aparca:** Un Change sorprendido a mitad de una promoción (`promoting`) se aparca con un
mensaje que pide revisar primero producción, para que nada se despliegue dos veces. Cuando
`observe` falla, se ejecuta `rollback` y el Change pasa a `rolled-back` con una petición de
Hotfix.

### Cuándo dice que no

Mensajes reales, primero los más frecuentes.

Respondes polish sin decir qué pulir:

```text
error: polish needs your notes: gdd resume cart-total --g4 polish --note ...
```

Respondes a un Change que no está esperando en G4:

```text
error: Change cart-total is not paused at G4.
```

Alguien intenta promover un Change que necesita un G4 humano antes de que la persona
responda:

```text
error: This change needs a human G4 before promotion.
```

Producción empeora tras la promoción: `observe` falla (aquí imprime `error rate up`), gdd
revierte y pide un Hotfix:

```text
error: release.observe failed (exit code 1):
error rate up
--- end of output ---
error rate up
Rollback: ok
Production failed while observing: rolled back to the previous version. A hotfix is needed. Message in .gdd/changes/cart-discount/inbox.md
```

## Learn

Learn (fase 5) no tiene puerta: solo endurece.

- `gdd learn` ejecuta una vez las herramientas de `[learn]` de `gdd/commands.toml`
  (duplicación, código sin usar), guarda en `.gdd/learn/` lo que dijo cada una, y lista las
  notas de polish que dio la persona en G4 y las clases de hallazgo que se repiten en las
  revisiones KO del Reviewer de dos Changes o más. Sale con 0 encuentren lo que encuentren las
  herramientas.
- El Orchestrator (comando `gdd-learn`) convierte cada error repetido en una regla, un lint, un
  hook, un test o una línea de `docs/standards.md`, y nunca afloja una puerta. Una propuesta
  que cambia `gdd/` es roja: decide la persona.

## Spikes

Un Change de tipo `Spike` responde a una pregunta y su código nunca llega a main.

| Paso | Qué pasa |
|---|---|
| Preparación | Solo hace falta G0; las zonas rojas pueden seguir pendientes. Sin G1, sin plan, sin modo test, sin mutation testing, sin Reviewer. Su color sigue eligiendo su Builder. |
| Build | Rondas del modo build (`mode=spike`) dentro de `builder_retries`, después las replanificaciones, nunca el modo test. |
| G2 | Path Guard y un ADR: la rama añade o cambia `docs/adr/*.md` con una línea `## Decision`. No se ejecuta ningún comando de `gdd/commands.toml`. |
| Presupuesto | Tras cada llamada de agente: el Spike se aparca en cuanto sus llamadas cuestan `[spike] max_usd` (5 USD). |
| G3, fusión, pre | Ninguno. |
| G4 | Siempre la persona. Un OK lleva a main solo el ADR; la rama del Spike se queda. |

Un Spike con variantes (`--option` una vez por opción, o `--variants <n>`, de 2 a 4) construye
cada variante en su propia rama `change/<id>-v<n>`, una tras otra, cada una con el presupuesto
entero. Se pausa en G4 cuando al menos una variante ha pasado su puerta, y se aparca cuando
ninguna lo ha hecho (`no variant passed its gate`); `gdd resume <id> --g4 ok --variant <n>`
trae el ADR de esa variante.

## Qué aparca un Change

Un Change aparcado se detiene con un mensaje en su Decision Inbox y conserva su worktree;
relánzalo con `gdd launch <id>` cuando hayas arreglado la causa.

| Causa | Dónde |
|---|---|
| G2 todavía en rojo tras las rondas y las replanificaciones | [G2](#g2-loop-gate) |
| Path Guard falló tras el modo test | [G2](#g2-loop-gate) |
| G3 todavía en rojo tras una corrección; KO del Reviewer o G3 en rojo tras una corrección | [G3](#g3-merge-gate) |
| Conflicto de fusión con el último main, o G3 en rojo sobre él | [G3](#g3-merge-gate) |
| El checkout principal no puede recibir la fusión: no está en la rama principal, o hay cambios sin commit en ficheros con seguimiento | [G3](#g3-merge-gate) |
| Un agente falló, o no respondió dentro de `[limits] agent_timeout_s` | cualquier llamada de agente |
| Un agente cambió el estado de GDD, o el Reviewer cambió un fichero | cualquier llamada de agente |
| Un Spike alcanzó su presupuesto, o ninguna variante pasó su puerta | [Spikes](#spikes) |
| Sorprendido a mitad de una promoción | [G4](#g4-release-gate) |
| `gdd park <id> <reason>` | a mano |

## Ver también

- [Cómo elige el riesgo las puertas](../explanation/risk.md): por qué gana el color y cuánto
  cuesta cada color.
- [Configuración](configuration.md#rulestoml): las claves detrás de cada fila de estas tablas.
- [Roles](roles.md): quién ejecuta cada paso y qué puede tocar cada uno.
- [Responder a la puerta de salida](../how-to/release-gate.md): OK, KO o polish, paso a paso.
