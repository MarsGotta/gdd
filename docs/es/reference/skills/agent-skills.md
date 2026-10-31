# Skills de los agentes de gdd

Cuando gdd ejecuta un Change, lanza agentes él mismo, desatendidos (*headless*: sin
conversación y sin nadie que responda), y entrega a cada uno una skill: al Builder en modo test,
al Builder en modo build, al Reviewer y, cuando el build se atasca, al Orchestrator para una
replanificación. Tú nunca ejecutas estas skills. Esta página enumera qué se le pide a cada una;
consúltala cuando leas su trabajo en la Decision Inbox y en los informes de las puertas.

## Cómo las reparte gdd

[`gdd launch`](../cli/launch.md) y [`gdd resume`](../cli/resume.md) arrancan cada agente con su
skill y una entrada corta que escribe gdd, como `change=export-csv mode=build`. El agente trabaja
en el worktree del Change, en su rama, sin nadie que responda a sus preguntas. El estado de GDD
no está en ese worktree: vive en `.gdd/` del checkout principal, cuya ruta gdd pasa en la
variable `GDD_STATE`. Los agentes leen ahí el estado y solo lo cambian con comandos de gdd.

Qué agente y qué modelo hacen cada rol sale de `gdd/models.toml`; el Reviewer siempre es de otra
familia de modelos que el Builder. Lo que puede ejecutar cada rol se fija por agente, en las
fichas de agente que lleva gdd dentro: consulta
[Permisos de los agentes](../roles.md#permisos-de-los-agentes).

gdd también arranca cada agente con su rol en la variable `GDD_ROLE`. Un agente que ha lanzado
gdd solo puede ejecutar `gdd g2`, `guard`, `note`, `status`, `classify`, `inbox`, `report` y
`scope`; gdd rechaza cualquier otro comando que intente. Consulta
[Lo que puede ejecutar un agente](../roles.md#qué-puede-ejecutar-un-agente).

| Skill | Rol | gdd la lanza | Entrada después de `change=<id>` |
|---|---|---|---|
| [`gdd-build-test`](#gdd-build-test) | Builder, modo test | primero, cuando el Change lleva los tests primero; otra vez tras una replanificación que dice `NEXT: test`, o tras un KO del Reviewer con un hallazgo en un fichero que solo puede escribir el modo test | `mode=test`, quizá `after=replan` o `fix=review` |
| [`gdd-build`](#gdd-build) | Builder, modo build | en cada ronda contra G2, y en cada corrección | `mode=build` o `mode=spike`, quizá `fix=g3`, `fix=review`, `fix=polish`, `after=replan`, `variant=<n>` |
| [`gdd-review`](#gdd-review) | Reviewer | después de que pase G3 | `round=1` o `round=2` |
| [`gdd-replan`](#gdd-replan) | Orchestrator, desatendido | cuando G2 sigue en rojo tras los reintentos del Builder | `reason=g2` |

Cada una habla en el idioma de `gdd/project.toml`. Los Builders y el Reviewer leen primero las
instrucciones de stack, `gdd-stack-<name>`, de cada stack del proyecto.

## gdd-build-test

El Builder en modo test escribe los pasos de los escenarios y los tests, antes de cualquier
código.

- **Lee** el paquete de contexto `.gdd/changes/<id>/package.md`, los ficheros `.feature` que
  nombra y el código bajo sus rutas, solo para leerlo.
- **Escribe** las definiciones de los pasos con las palabras del dominio, con una aserción real
  en cada paso `Then`. Reutiliza un paso que ya dice lo mismo, y solo añade uno nuevo cuando
  ninguno encaja, con sus valores como parámetros. Cada paso ejercita el comportamiento en la
  capa más baja que lo muestra: una función del dominio, después la línea de comandos, después
  la pantalla.
- **En un Change amarillo o rojo** que toca código existente, escribe primero tests de
  caracterización que fijan lo que hace hoy el código.
- **Comprueba** con [`gdd g2 --role builder-test`](../cli/g2.md): los tests nuevos tienen que
  fallar porque falta el comportamiento, no por un error de sintaxis o de configuración.
- **Tras una replanificación** (`after=replan`), arregla solo lo que nombra `replan.md`, en los
  pasos y en los tests.
- **Tras un KO del Reviewer** (`fix=review`), arregla solo los hallazgos sobre los ficheros que
  escribe el modo test (los pasos de los escenarios y los tests escritos primero), en cada sitio
  de la misma clase. El modo build arregla el resto.
- **Nunca** cambia un fichero `.feature` (los aprobaste en G1), escribe código de producción,
  toca rutas protegidas, ni se salta o silencia tests.
- **Termina** con un resumen de tres líneas: qué tests ha escrito y por qué fallan ahora.

## gdd-build

El Builder en modo build escribe el código hasta que pasa la puerta del bucle (G2, *Loop Gate*).

- **Lee** el paquete, `docs/standards.md` cuando existe y, según diga la entrada, los fallos de
  la ronda anterior (`g2.json`), los de G3 (`g3.json`, con `fix=g3`), los motivos del Reviewer
  para un KO (`review.md`, con `fix=review`), la replanificación (`replan.md`, con
  `after=replan`) o tus notas de polish de G4 en la bandeja (`fix=polish`).
- **Escribe** el código del dominio, su conexión con la aplicación y los tests unitarios que
  necesita. Reutiliza lo que el paquete dice que reutilice y no añade ninguna dependencia:
  cuando parece necesaria una, deja una nota.
- **Comprueba** con [`gdd g2 --role builder-build`](../cli/g2.md) y corrige hasta que G2 está en
  verde, o hasta que ve que no puede.
- **Ante un KO del Reviewer**, corrige cada sitio con la misma clase de bug de cada hallazgo, no
  solo la línea nombrada, y enumera los sitios que ha cambiado para cada hallazgo.
- **Nunca** edita los tests de aceptación ni los ficheros `.feature`, toca rutas protegidas,
  silencia avisos ni se salta comprobaciones, ni escribe casos especiales para los ejemplos de
  los escenarios. Un fichero fuera de las rutas del paquete tiene que quedarse en el color del
  Change o en uno inferior, y va con una nota `decided`; una ruta de un color superior hace
  fallar la comprobación de alcance de G2.
- **Cuando algo es ambiguo**, elige la opción reversible más sencilla y la anota con
  [`gdd note`](../cli/note.md): `decided`, o `needs` cuando tienes que decidir tú.
- **Termina** con un resumen de tres líneas: qué ha cambiado y el estado de G2.

Con `mode=spike`, el Change es un Spike: el Builder construye lo más pequeño que responde a su
pregunta, lo mide y escribe un ADR en `docs/adr/` con una sección `## Decision`. G2 es entonces
Path Guard y ese ADR, y el Spike se aparca en cuanto sus llamadas cuestan lo que permite
`[spike] max_usd` en `gdd/rules.toml`. Con `variant=<n>`, construye solo la opción de su
variante. Consulta [Explorar con un Spike](../../how-to/spikes.md).

## gdd-review

El Reviewer, un modelo de otra familia que el Builder, lee el Change entero y responde OK o KO.
Solo lee: si cambia un fichero, gdd aparca el Change.

- **Lee** el paquete, el registro `change.json`, los ficheros `.feature` del Change, el diff
  entero contra `main`, los informes de G2 y G3 y `docs/standards.md`.
- **Comprueba** que cada paso hace lo que dice su texto, con aserciones reales; que no hay
  casos especiales ni atajos para que pasen los tests; que el Change se queda dentro de su
  alcance; en un Change rojo, la seguridad (datos, permisos, secretos, entradas sin validar); el
  trabajo que solo parece terminado; y los pasos nuevos que repiten uno existente.
- **Profundidad**: `review` en la clasificación del Change dice `quick` (los ficheros tocados, el
  paquete y los escenarios) o `deep` (además el código que llama a lo que ha cambiado, los
  caminos que siguen sus datos, los agujeros de seguridad, la pérdida de datos, la concurrencia y
  cada zona roja tocada). Los Changes rojos siempre reciben una revisión deep.
- **Revisiones anteriores**: una revisión posterior dice, para cada hallazgo bloqueante
  anterior, si está corregido, y busca hallazgos nuevos solo en los ficheros que han cambiado
  desde entonces. La ronda 2 comprueba los motivos del KO de la ronda 1 y no abre ningún tema
  nuevo salvo que bloquee.
- **Responde** con los hallazgos como `F<n> · <class> · file:line · what fails · other places`,
  y los que no bloquean bajo «Improvements». Con un OK añade una sección `## How it works`, de 3 a
  6 líneas sencillas sobre dónde está el Change y cómo encaja: es el *How it works* que lees en la
  Decision Inbox y en la sentada de G4. La última línea es `VERDICT: OK` o `VERDICT: KO`.

## gdd-replan

El Orchestrator, sin conversación, replanifica cuando el Builder ha agotado sus reintentos y G2
sigue en rojo.

- **Lee** el paquete, `g2.json` y las salidas del Builder en `.gdd/changes/<id>/outputs/`.
- **Decide la causa**: una tarea demasiado grande, un paquete con muy poco contexto, un enfoque
  equivocado o un problema del arnés.
- **Escribe** `.gdd/changes/<id>/replan.md`: la causa en una frase, el enfoque nuevo en pasos
  concretos y el contexto que le faltaba al paquete. El paquete en sí se queda como está.
- **Termina** `replan.md` con `NEXT: test`, cuando el fallo está en los pasos o en los tests del
  modo test, o con `NEXT: build`. Con `NEXT: test`, gdd ejecuta primero una ronda del modo test,
  y en los dos casos después una ronda más del modo build.
- **Cuando solo tú puedes arreglarlo**, un problema del arnés o una decisión, deja una nota
  `needs` en vez de una corrección. Cuando G2 sigue en rojo tras la última ronda del modo build,
  el Change se aparca, y la nota está en su Decision Inbox.
- **Nunca** escribe código, afloja puertas, cambia `gdd/` ni toca tests aprobados.

## Instrucciones de stack

[`gdd agents install`](../cli/agents-install.md) también escribe una skill por cada stack de
`stack` en `gdd/project.toml`, llamada `gdd-stack-<name>`, como `gdd-stack-python`: las
instrucciones propias de ese stack. Los Builders y el Reviewer la leen antes de trabajar.
[Stacks](../stacks.md) enumera lo que trae cada ficha de stack.

## Ver también

- [Skills](index.md): las skills que ejecutas tú.
- [Roles](../roles.md): quién construye, quién juzga y qué no puede hacer cada uno.
- [La vida de un Change](../../explanation/life-of-a-change.md): cuándo se ejecuta cada agente en
  la receta.
- [Informes de puerta](../files.md#informes-de-puerta): `g2.json`, `g3.json` y los demás ficheros
  que leen los agentes.
