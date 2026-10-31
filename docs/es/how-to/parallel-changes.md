# Ejecutar Changes en paralelo

Construye un segundo Change mientras el primero todavía se está construyendo. Cada Change tiene
su propia carpeta de trabajo, se fusionan de uno en uno, y gdd limita cuántos se construyen a la
vez y te avisa cuando dos planean los mismos ficheros. Arrancas cada Change como siempre.

## Antes de empezar

- Un primer Change ya lanzado: consulta [Describir un Change a tu agente](run-a-change.md).
- Recursos suficientes en tu máquina para dos builds a la vez, y suscripciones de agentes que
  admitan dos sesiones simultáneas. El mutation testing (pruebas de mutación) y la ejecución
  completa de escenarios de G3 son los pasos más pesados.

Los ejemplos usan el proyecto `notes` y los Changes `export-csv` y `login-rate-limit`.

## Arrancar un segundo Change

Abre otra sesión de tu agente en la carpeta del proyecto y describe ahí el segundo Change, como
hiciste con el primero:

```text
/gdd-change limit login attempts, five per minute per account
```

El Orchestrator lo prepara y celebra su sentada mientras el primer Change se construye. Sigue
los dos en el Board (`gdd board`): cada uno aparece en Build y después en Release.

## Cómo comparten un proyecto varios Changes

Dos Builders editando la misma carpeta se sobrescribirían los ficheros. Por eso cada lanzamiento crea
un [worktree](../glossary.md#worktree) para su Change: una segunda carpeta de trabajo del mismo
repositorio, en la rama `change/<id>`, por defecto junto al proyecto
(`../notes.worktrees/export-csv`). El checkout principal, tu carpeta de siempre del proyecto, se
queda en main, así que tú y el Orchestrator seguís trabajando ahí. Todos comparten un único
estado: el `.gdd/` del checkout principal guarda los registros, el log y las aprobaciones de
todos los worktrees.

Lee el diagrama de arriba abajo: los dos Changes se construyen a la vez, y el segundo en
terminar toma el main más reciente y vuelve a pasar G3 antes de fusionarse.

```mermaid
sequenceDiagram
    participant A as export-csv
    participant M as main
    participant B as login-rate-limit
    A->>A: Build, G2, G3, Reviewer
    B->>B: Build, G2, G3, Reviewer
    A->>M: fusión (tiene el cerrojo de fusión)
    Note over B: esperando la fusión de otro Change
    M->>B: main ha avanzado: fusiona el main más reciente en la rama
    B->>B: G3 otra vez sobre el main más reciente
    B->>M: fusión
```

## 1. Fija cuántos Changes se construyen a la vez

El límite es `[parallel] max` en `gdd/rules.toml`, 2 por defecto. `gdd init` no escribe esa
tabla. Para cambiar el límite, añádela en main, con la carpeta de los worktrees si quieres otra:

```toml
[parallel]
max = 3                 # changes building at once; a whole number from 1
folder = "../trees"     # where the worktrees go, from the project root
```

Solo cuentan los Changes que se están construyendo: `prepared`, `building` y `polishing`. Un
Change aparcado o que espera en G4 no ocupa plaza. gdd rechaza un lanzamiento que supera el
límite antes de ejecutar nada, y nombra los Changes que se están construyendo y lo que permitiría
empezar al nuevo. El mensaje tiene esta forma (ilustrativo: montado a partir del código, no de
una ejecución real):

```text
2 change(s) building, the cap is 2: cart-total, export-csv
login-rate-limit can launch once one of those building reaches G4 or is parked, or with a higher [parallel] max in gdd/rules.toml.
```

El límite real suele estar por debajo del número que escribes. Dos ejecuciones de G3 a la vez
pueden llenar la memoria de una máquina pequeña, las suscripciones de los agentes tienen cuotas
y tú solo puedes celebrar unas pocas sentadas a la vez.

## 2. Lee los avisos de solapamiento

Cuando se lanza un Change, gdd compara las rutas que planea con las rutas de todos los Changes
en curso: en construcción, aparcados o esperando en G4 (los Changes cerrados no cuentan). Un
solapamiento no detiene el lanzamiento. gdd avisa, nombra las rutas y guarda el aviso en la
Decision Inbox, en "Decided without you". Uno real, del propio repositorio de gdd:

```text
builder-session overlaps mutation-speed (in-pre): crates/gdd-acceptance/tests/acceptance.rs with crates/gdd-acceptance/tests/acceptance.rs; git settles it when builder-session merges.
```

La línea nombra los dos Changes, el estado del otro (`in-pre`) y el fichero que comparten. Así
compara gdd las rutas:

| Rutas planeadas | ¿Se solapan? |
|---|---|
| `src/export.rs` y `src/export.rs` | Sí: es el mismo fichero |
| `src/export.rs` y `src/store.rs` | No: dos rutas simples solo se solapan cuando son la misma |
| `src/lib/**` y `src/lib/parse.rs` | Sí: un patrón de carpeta se solapa con todas las rutas que contiene |
| `src/**` y `docs/**` | No: se separan antes del primer comodín |

gdd solo compara las rutas planeadas. Un Change que toca un fichero que no había planeado puede
chocar igualmente con otro en la fusión.

## 3. Sigue la cola de fusión

Las fusiones van de una en una, bajo un cerrojo: solo puede fusionarse el Change que lo tiene.
Un Change que llega a su fusión mientras otro se está fusionando dice
`waiting for the merge of another change` y continúa cuando el cerrojo queda libre.

Cuando main ha avanzado desde que un Change pasó G3, gdd no lo fusiona sobre un main antiguo:

1. gdd hace commit del trabajo del Change y fusiona el main más reciente en su rama, nunca con
   un rebase.
2. Dice `main moved under export-csv: G3 again on top of it` y vuelve a ejecutar G3 ahí.
3. Si está en verde, el Change se fusiona. Si está en rojo, se aparca con
   `G3 red on top of the latest main`. Cuando git no puede fusionar main en la rama, se aparca
   con `main moved under it and the merge has conflicts in <paths>` y la forma de salir:
   consulta [Resolver los conflictos de fusión con
   main](parked-change.md#6-resuelve-los-conflictos-de-fusión-con-main).

Cualquier commit en main mientras se construye un Change cuenta como que main ha avanzado,
incluido un commit de documentación. Ese Change ejecuta entonces G3 una segunda vez antes de
fusionarse.

## 4. Aprueba cosas mientras trabajan los agentes

Aprobar las zonas rojas, abrir G0 o guardar el autotest escribe un estado que comparten todos
los Changes. Mientras hay una llamada de agente en marcha, esa escritura espera a que termine,
y entretanto no empieza ninguna llamada nueva. Verás:

```text
waiting for the agents at work to end their call
G0 ok (machines).
```

La primera línea es la espera. La segunda llega cuando la llamada ha terminado y gdd ha escrito
la aprobación. Aprobar las zonas rojas tal como ya están no escribe nada, así que nunca espera.

## Buenas prácticas

- No ejecutes a la vez dos Changes que tocan los mismos ficheros centrales. Antes de lanzar
  un segundo Change, compara las `paths` de los dos registros. Los ficheros que toca casi
  cualquier Change (el punto de entrada de la línea de comandos, el fichero de pasos de los
  escenarios) son el origen de los conflictos: dos Changes que añaden pasos al final del mismo
  fichero siempre entran en conflicto.
- Encadena los Changes que reestructuran la misma función. Si los dos remodelan una misma
  función, quedarte con un lado del conflicto hace perder una funcionalidad. Lanza el segundo
  cuando el primero ya se haya fusionado.
- Planifica las rutas con honestidad. Los avisos de solapamiento y la comprobación de
  alcance solo conocen lo que el Change planea.
- Agrupa los commits en main. Cada commit mientras se construyen Changes obliga a cada uno
  a pasar G3 una segunda vez. Deja aparte el trabajo no relacionado (una rama de documentación,
  por ejemplo) y fusiónalo en un momento tranquilo.
- **No toques `.gdd/bin/` mientras trabaja un agente.** gdd comprueba esa carpeta antes y
  después de cada llamada de agente, en todos los Changes: un binario nuevo ahí en mitad de una
  llamada aparca el Change. Cámbialo cuando no se esté construyendo ningún Change.

## Por dentro

Cada lanzamiento es un [`gdd launch`](../reference/cli/launch.md): rechaza un Change que supera
el límite antes de ejecutar nada, y avisa primero de los solapamientos, también en la Decision
Inbox. La cola de fusión funciona dentro de la receta de cada Change, y
[`gdd status`](../reference/cli/status.md) y [`gdd board`](../reference/cli/board.md) muestran
todos los Changes en curso.

## Siguientes pasos

- [Volver a poner en marcha un Change aparcado](parked-change.md): qué hacer cuando una fusión
  aparca un Change.
- [Describir un Change a tu agente](run-a-change.md): lanzar, seguir en el Board y relanzar.
- [Configuración, rules.toml](../reference/configuration.md#parallel): `[parallel]`, el límite
  y la carpeta de los worktrees.
- [La vida de un Change](../explanation/life-of-a-change.md): en qué punto de la receta está la
  cola de fusión.
