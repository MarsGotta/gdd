# Revisar un Change en G4

Usa esta página cuando un Change te espera en la puerta de salida (G4, *Release Gate*): el
Orchestrator lo trae de vuelta al chat, o el Board lo muestra como `waiting for your G4`. Para
entonces, las máquinas ya han comprobado que el Change funciona. G4 es donde compruebas que es
lo que querías, antes de que llegue a producción. Miras lo que hace y respondes OK, KO o
polish, con palabras.

## Cuándo te espera un Change

G4 es la única parada después de la construcción, y solo algunos Changes llegan a ella. Un
Change espera a una persona en G4 cuando:

- tiene G1, porque crea o modifica un escenario (`g4_if_g1 = true` en la vía completa de su
  color en `gdd/rules.toml`, el valor por defecto); o
- es rojo: un Change rojo siempre se encuentra con una persona en G4, y gdd rechaza un fichero
  de reglas que lo desactive.

A cualquier otro Change lo promueven las máquinas en cuanto pre está en verde. Un Spike también
se para en G4, donde eliges lo que dice su ADR: consulta [Explorar con un Spike](spikes.md).

Cuando un Change llega a esperarte, ya ha pasado mucho sin ti. Ha superado la puerta del bucle
(G2, *Loop Gate*) y la puerta de integración (G3, *Merge Gate*), y el Reviewer ha dicho OK.
Después gdd lo ha fusionado en `main`, con su registro en el commit de fusión, lo ha construido
una vez y ha desplegado esa build en pre, la copia del proyecto que se ejecuta antes de
producción. Pre ha ejecutado sus tests y la demo: vídeo, capturas de pantalla o, en una
biblioteca o una herramienta sin pantalla, la ejecución legible de sus escenarios, que queda
en `.gdd/changes/CHANGE/demo/`. Lo que apruebas es esa build, tal como funciona en pre.

`gdd init` deja vacíos todos los pasos de publicación, con `simulated = true`, en la tabla
`[release]` de `gdd/commands.toml`: hasta que rellenes `deploy_pre`, `demo` y los demás, pre y
producción son simulados y no hay demo que mirar.

## 1. Lee la Decision Inbox

Cuando el Change se detiene en G4, el Orchestrator trae al chat su Decision Inbox y su demo,
con palabras sencillas, y te pide la respuesta. Para `cart-total`, el Change que muestra el
total del carrito de `shop`, el agente escribió:

<!-- The chat excerpts and the screens of this page come from one real run of gdd 0.1.0 (main
at e244577) on 2026-10-31, with Claude Code as the chat agent and Codex as the Reviewer: the
run of the tutorial. Paths are shown under /home/me. -->

```text
The **cart-total** change is built and checked, and it's waiting for your **G4** answer.

**What it does:** a cart adds up the pieces in it and shows two figures, the total and how much
of that total is tax.
- 1 Blue mug (24.20) + 2 Wide bowls (36.30 each) gives a **total of 96.80, of which 16.80 is
  tax**.
...
**Checks:**
- **All 6 scenarios pass**, including the 2 hidden examples the Builder never saw. That shows
  it implemented the rule rather than copying the numbers.
- **The other merge checks pass:** lint, types, unit tests, security, secrets and dependencies.
- **Mutation testing was skipped,** as the rules say for yellow changes. ...
- **The Reviewer from another model family said OK** with no findings.
- **It's merged into `main`.** The test server is simulated, so there's no page or video to
  look at yet. ...

**Two things you should know:**
- The inbox asks you to approve the cart's technical interface: how other code adds pieces and
  reads the totals. That's a code detail; I've checked it and it's fine, so you don't need to
  decide anything there.
- The cart doesn't yet refuse odd input like a quantity of 0 or a negative price. Nothing
  allows that today, but I'd add the rule when pieces are added from a real page.

**Next step:** reply **OK** to send it to production (simulated for now), **KO** to reject it,
or **polish** with what you'd like changed.
```

Dice lo que hace el Change con los números de tus escenarios, lo que han dicho las puertas y el
Reviewer, y lo que ha leído para ti en la bandeja. Los 6 escenarios son los 3 visibles de
`cart-total`, sus 2 ejemplos ocultos y el escenario de humo que escribió la fase Setup
(`features/smoke.feature`): G3 ejecuta todos los escenarios del proyecto, no solo los del
Change.

La bandeja en sí es el único mensaje que gdd guarda por cada Change, en
`.gdd/changes/CHANGE/inbox.md`. En G4 empieza por cómo funciona el Change. La bandeja completa
de `cart-total`:

```text
# Decision Inbox · cart-total · Show the cart total, taxes included

Color yellow · lane full · state in-pre

## How it works
- `src/shop/cart.py` adds a plain Python cart, independent of the web layer.
- Totals sum tax-inclusive prices multiplied by quantities using `Decimal`.
- Included tax uses the supplied rate, calculated once on the total and rounded half up.
- Acceptance steps follow the existing fixture pattern; unit tests cover rounding, alternative rates and empty carts.

## Needs your decision
- API expected by the tests: shop.cart.Cart(tax_rate: Decimal) with add(name: str, price: Decimal, quantity: int), and Decimal properties total and tax_included, both quantized to two places (str() shows e.g. 96.80). The rate is a percentage (Decimal(21)).
- G4 · Release Gate: check the pre demo and the change message, then answer with gdd resume cart-total --g4 ok (promote to production), --g4 ko (reject: production untouched) or --g4 polish --note <what to polish> (one more Builder round).

## Decided without you
- No human plan: the change is yellow. Money as Decimal, tax rate given to the cart (not hard-coded), tax computed once on the cart total and rounded half up to the cent.
- Euros: scenarios keep amounts without the euro sign, as approved; the currency shows up when the cart reaches a page.

## Parked
- Nothing.
```

- **How it works** (cómo funciona) es la explicación breve del Reviewer sobre el código que ha
  tocado el Change: dónde está, cómo encaja, el patrón que sigue. Te deja aprobar lo que
  entiendes sin leer cada línea. Cuando el Reviewer no la ha escrito, la bandeja lo dice.
- **Needs your decision** (necesita tu decisión) recoge la petición de G4 y las notas que los
  agentes te han dejado por el camino. Aquí el Builder en modo test describió el código que
  esperan sus tests, y el Orchestrator lo comprobó y te lo resumió. La línea de G4 nombra el
  comando en que se convierte tu respuesta: lo ejecuta el Orchestrator, tú respondes con
  palabras.
- **Decided without you** (decidido sin ti) lista lo que han elegido los agentes por su cuenta,
  y las rutas fuera del plan que ha aceptado la comprobación de alcance.
- **Parked** (aparcado) lista lo que un agente ha dejado para más tarde: aquí, nada.

Cuando corresponde, siguen más secciones: las excepciones de seguridad que añade el Change, la
deuda conocida que sigue fallando o que se ha pagado, y tus notas de polish anteriores.

## 2. O abre la sentada de G4

Si prefieres una sola pantalla con casi todo aquello por lo que juzgas el Change, abre la
sentada en una terminal:

```bash
gdd sitting cart-total
```

Deberías ver (sin las líneas en blanco):

```text
┌ gdd · Sitting · shop ────────────────────────────────────────────────────────────────────────────┐
│cart-total · Show the cart total, taxes included · waiting for your G4                            │
│How it works                                                                                      │
│- `src/shop/cart.py` adds a plain Python cart, independent of the web layer.                      │
│- Totals sum tax-inclusive prices multiplied by quantities using `Decimal`.                       │
│- Included tax uses the supplied rate, calculated once on the total and rounded half up.          │
│- Acceptance steps follow the existing fixture pattern; unit tests cover rounding, alternative    │
│rates and empty carts.                                                                            │
│Reviewer: OK                                                                                      │
│No blocking findings. The implementation matches the package, and scenario steps assert exact     │
│amounts with two decimal places. No duplicated steps or example-specific shortcuts found.         │
│Recorded G2 and G3 pass; mutation testing was skipped. This review was read-only; no tests were   │
│rerun.                                                                                            │
│Demo: none, the project has no demo command                                                       │
│Decided without you                                                                               │
│- No human plan: the change is yellow. Money as Decimal, tax rate given to the cart (not          │
│hard-coded), tax computed once on the cart total and rounded half up to the cent.                 │
│- Euros: scenarios keep amounts without the euro sign, as approved; the currency shows up when the│
│cart reaches a page.                                                                              │
│Changelog: none set                                                                               │
│Flow time 3m · active 2m (59%) · waiting 1m · between steps 0s                                    │
│Agents: builder-test · sonnet 51s, reviewer · gpt-6-astra 30s, builder-build · sonnet 17s         │
│Gates: security 1s, secrets 0s, acceptance 0s, acceptance_full 0s, unit 0s, architecture 0s, lint │
│0s, types 0s, dependencies 0s, lockfile 0s, path-guard 0s                                         │
│Waiting: at the sitting 55s, to start 15s                                                         │
│↑↓ scroll · o OK · k KO · p polish · q quit                                                       │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

De arriba abajo: cómo funciona el Change, el veredicto del Reviewer y sus palabras, la demo
(con sus números cuando el comando de demo los ha escrito, por ejemplo
`Demo: 12 scenarios (12 passed)`), lo que han decidido los agentes sin ti, la línea del
changelog y en qué se ha ido el tiempo del Change. Aquí el Builder en modo test es el que más
ha tardado, y el Change esperó 55 segundos tu OK en la primera sentada.

Dos líneas dicen "nada todavía". `Demo: none, the project has no demo command` porque `shop`
aún no ha rellenado el paso `demo` de `[release]`, así que pre no ha dejado ficheros de demo.
`Changelog: none set` significa que nadie ha puesto una: entonces el commit de fusión lleva la
de por defecto, el título bajo la sección del tipo del Change (`Added` en una Feature).

Las notas de los agentes en *Needs your decision* y *Parked* no aparecen en la sentada: léelas
antes en la bandeja.

## 3. Comprueba antes de decir OK

Todas las puertas han pasado, así que la pregunta en G4 ya no es si el Change funciona sino si
es lo que quieres. Antes de responder, mira:

1. **La demo en pre.** ¿Hace lo que prometían los escenarios, en la interfaz real?
2. **How it works.** ¿La explicación encaja con lo que pediste, y con la forma que quieres que
   mantenga el código?
3. **Las palabras del Reviewer.** Un OK puede traer observaciones que merecen un polish.
4. **Decided without you.** Cada decisión es una que podrías haber tomado de otra forma.
5. **Las excepciones de seguridad y la deuda conocida**, cuando la bandeja las lista.

Si algo se acerca pero no está bien, responde polish en lugar de KO: el polish conserva el
trabajo.

## 4. Responde

Responde al Orchestrator en el chat, con tus palabras. Registra tu respuesta por ti.

| Dices | Qué pasa |
|---|---|
| OK | El Change pasa a producción. |
| KO, y por qué | El Change se rechaza y se queda fuera de producción. |
| Polish, y qué pulir | Una ronda más del Builder con tus palabras, después otra vez las puertas y la demo, y el Change vuelve a esperar en G4. |

Para `cart-total`, la respuesta fue una palabra:

```text
OK
```

El agente respondió:

```text
**cart-total** is released as **v0.1.0**, and the change is closed. Production is still
simulated, so nothing is live on the web yet. The shop's core now has a cart that shows the
total and the tax included in it.

**What it cost:** the report shows **$0.39** in total. That figure covers only the Builder's
two rounds (writing the tests, then the code). The Reviewer and my own work show $0.00, which
looks like they weren't priced in the report rather than free. Your answers stopped the process
twice for a human decision: the scenarios and G4.
...
```

Un polish necesita tus palabras, por ejemplo `Polish: show the total with the euro sign.` Un KO
necesita el motivo.

En la sentada, `o` es OK, `k` KO y `p` polish. `k` y `p` preguntan primero
`What should change?` y esperan tus palabras. Después, cada respuesta pasa por una confirmación
que muestra el comando exacto y lo que sigue. Al pulsar `o` aparece:

```text
┌ gdd · Sitting · shop ────────────────────────────────────────────────────────────────────────────┐
│This runs, as you:                                                                                │
│gdd resume cart-total --g4 ok                                                                     │
│The change is promoted.                                                                           │
│y to run · n to go back                                                                           │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`y` cierra la pantalla y ejecuta ese comando en la terminal, así que ves su progreso; `n`
vuelve atrás y no se ejecuta nada. KO dice "The change is rejected and stays out of
production."; polish dice "One more Builder round with your words, then the gates and the demo
again." Solo una persona puede responder a G4: a un Builder o un Reviewer que lo intente se le
rechaza.

### OK: promuévelo

gdd registra tu OK, promueve a producción **la misma build** que se ejecutó en pre, sin volver
a construirla, y después observa producción. Cuando la observación pasa, el Change se cierra y
su worktree desaparece; su rama se queda. Para `cart-total`, gdd imprimió:

```text
Released v0.1.0
Change cart-total closed. Message: .gdd/changes/cart-total/inbox.md · Tokens: gdd report --change cart-total
```

- **Versiones.** Con `[versions] mode = "auto"` (el valor por defecto), un Change promovido con
  línea de changelog recibe la siguiente etiqueta de versión en cuanto pasa la promoción: aquí
  `v0.1.0`, la primera versión de un proyecto, en el commit de fusión de `cart-total`. Una
  Feature posterior sube la versión menor. Consulta [Publicar versiones y mantener un
  changelog](releases-and-changelog.md).
- **Revertir (*rollback*).** Cuando la observación falla, gdd ejecuta el paso `rollback` para
  volver a la versión anterior, el Change termina `rolled-back` y su bandeja pide un hotfix:
  "Production failed while observing: rolled back to the previous version. A hotfix is
  needed." Ejecuta `/gdd-hotfix` (`$gdd-hotfix` en Codex; consulta [la skill de
  hotfix](../reference/skills/hotfix.md)).

### KO: recházalo

gdd registra tu KO y tus palabras, deja producción intacta y borra el worktree. El Change
termina `rejected`.

KO no deshace la fusión. gdd fusionó el Change en `main` justo después del OK del Reviewer,
antes de pre y de G4, así que después de un KO `main` y pre lo siguen teniendo y producción no.
Un Change posterior se fusiona encima, así que su publicación también llevaría este código a
producción. Si quieres sacar el código de `main`, pide al Orchestrator un Change que lo quite.

### Polish: una ronda más

El Builder tiene una ronda más en modo build con tus palabras, y después el Change vuelve a
pasar G2, G3, el Reviewer, la fusión cuando hay algo nuevo y pre, y se detiene en G4.
Respóndele otra vez como antes. El polish sustituye a un KO en lugar de añadir una parada, y tus
palabras se quedan en la bandeja, en Polish. Learn lista las notas de polish que se repiten
entre Changes, para que se conviertan en una línea de `docs/standards.md` o en una comprobación
(consulta [Medir el trabajo](measure.md)).

## Cuando dice que no

- **Falla una promoción.** Producción queda intacta y el Change sigue en pausa en G4, con el
  motivo en su bandeja. Corrige la causa y vuelve a responder OK: el mismo OK lo reintenta.
- **Una promoción se corta a medias** (la máquina se paró a mitad). gdd aparca el Change en
  lugar de adivinar, con un mensaje para que mires primero producción, y así no se despliega
  nada dos veces.

<!-- The two refusals below were reproduced the same day in a throwaway copy of the same
project, without agents and without a launch. -->

- **Polish sin palabras.** gdd lo rechaza:

  ```text
  error: polish needs your notes: gdd resume cart-total --g4 polish --note ...
  ```

- **El Change ya no está esperando**, por ejemplo porque alguien lo ha respondido mientras
  tanto. gdd lo dice y no ejecuta nada. La sentada dice, para un Change que se ha cerrado
  mientras tanto, `CHANGE is closed now: nothing was run`; una respuesta en el chat recibe:

  ```text
  error: Change cart-total is not paused at G4.
  ```

## Por dentro

Tu respuesta se convierte en [`gdd resume CHANGE --g4 ok`](../reference/cli/resume.md),
`--g4 ko --note ...` o `--g4 polish --note ...`; las palabras después de `--note` son el resto
de la línea. En un Spike con variantes, `--variant N` dice qué variante lleva el OK a `main` o
vuelve a construir el polish. [`gdd inbox`](../reference/cli/inbox.md) imprime la Decision
Inbox, que también se guarda en `.gdd/changes/CHANGE/inbox.md`, y
[`gdd sitting`](../reference/cli/sitting.md) abre la pantalla de G4. La línea del changelog sale
de [`gdd changelog-line`](../reference/cli/changelog-line.md).

## Siguientes pasos

- [Volver a poner en marcha un Change aparcado](parked-change.md): cuando un Change se para
  antes de G4.
- [Publicar versiones y mantener un changelog](releases-and-changelog.md): etiquetas,
  `gdd release` y `CHANGELOG.md`.
- [Pocas paradas para la persona](../explanation/human-stops.md): por qué G4 es la única parada
  después de la construcción, y por qué hecho no es bueno.
