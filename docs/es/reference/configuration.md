# Configuración

Cada fichero de `gdd/`, la carpeta de los ajustes de un proyecto, y cada clave que gdd lee de
él: su tipo, el valor que gdd usa cuando falta la clave y lo que hace. Los ficheros son TOML.

## Cómo encajan los ficheros

`project.toml` describe el proyecto. `zones.toml` da un color a cada ruta, y `rules.toml` da a
cada color sus puertas, sus límites y sus rutas protegidas. `commands.toml` guarda los comandos
que ejecutan esas puertas, y `models.toml` el agente y el modelo de cada rol. `generated.toml`
sirve a las actualizaciones y `debt.toml` a los proyectos adoptados. El porqué de los colores está
en [Cómo elige el riesgo las puertas](../explanation/risk.md).

## Los ficheros

`gdd init` escribe los cinco primeros ficheros y `gdd/generated.toml`; `gdd adopt` escribe
`gdd/debt.toml`. Todos se suben al repositorio con el proyecto.

| Fichero | Lo escribe | Lo lee | Contiene |
|---|---|---|---|
| [`gdd/project.toml`](#projecttoml) | `gdd init` | `gdd status`, los comandos de stacks, los agentes | Lo básico del proyecto |
| [`gdd/zones.toml`](#zonestoml) | `gdd init`, después el Orchestrator | `gdd classify`, las puertas, la comprobación de alcance de Path Guard, la aprobación de las zonas rojas | El mapa de zonas: qué rutas tienen qué color |
| [`gdd/rules.toml`](#rulestoml) | `gdd init` | todo comando que clasifica, pasa puertas, lanza o fusiona | La tabla de reglas, los límites, las rutas protegidas |
| [`gdd/models.toml`](#modelstoml) | `gdd init` | `gdd prepare`, `gdd launch`, `gdd resume`, `gdd status`, `gdd agents` | El agente y el modelo de cada rol |
| [`gdd/commands.toml`](#commandstoml) | `gdd init` | las puertas, `gdd release`, `gdd selftest`, `gdd learn`, `gdd adopt`, `gdd ci` | Los comandos de las puertas, de Learn y de release |
| [`gdd/generated.toml`](#generatedtoml) | `gdd init`, `gdd upgrade` | `gdd upgrade` | Lo que `gdd init` escribió una vez y lo que se le pidió |
| [`gdd/debt.toml`](#debttoml) | `gdd adopt`, después cada fusión | las puertas, `gdd ci`, `gdd status` | La deuda conocida de un proyecto adoptado |

No existe `gdd/release.toml`: consulta [release.toml](#releasetoml).

Cuando gdd no puede analizar un fichero, el comando se detiene e imprime el nombre del fichero
y el error de TOML. gdd ignora las claves que no lee, salvo donde una sección de abajo diga otra
cosa.

## Quién puede cambiarlos

Toda ruta bajo `gdd/` es [arnés](../glossary.md#arnés), los ficheros que gdd usa para
comprobar el proyecto:

- `gdd init` pone `gdd/**` en `[protected] all` de `gdd/rules.toml`, así que
  [Path Guard](../glossary.md#path-guard) hace fallar a cualquier Builder o Reviewer que la
  toque, en G2 y en CI (`gdd ci` hace fallar una rama de Change que cambia una ruta de `all`).
- `gdd init` pone `gdd/**` en la zona roja del arnés de `gdd/zones.toml`, así que un Change que
  la planifica es rojo.

Estos ficheros cambian en la rama principal, fuera de cualquier Change, y solo cuando la
persona está de acuerdo: el Orchestrator propone, la persona decide. Las
[zonas rojas](gates.md#zonas-rojas) y el plan de un Change rojo se aprueban en la sentada. Una
edición de las zonas rojas de `gdd/zones.toml` necesita que se aprueben de nuevo.

Path Guard juzga un build con las listas `[protected]` de `gdd/rules.toml` tal como están ahora
y tal como estaban en el commit donde empezó el build. Borrar una protección, o el fichero
entero, no libera de ella a ningún rol.

## project.toml

Lo básico del proyecto. `gdd init` lo escribe a partir de sus flags. También marca la raíz del
proyecto: gdd busca `gdd/project.toml` en la carpeta donde se ejecuta y en las
carpetas superiores, y se niega a ejecutarse fuera de un proyecto (`not a GDD project`).

`gdd status` lee `name`, `language` y `platform`. Todo comando que necesita los stacks lee
`stack`. Los agentes leen `language` y `scenario_runner`, porque sus comandos se lo indican.

```toml
name = "notes"
description = "A command-line notes tool"
language = "en"   # language the agents use with the human
platform = "cli"   # mobile | desktop | web | cli | other
stack = ["python"]   # the stack sheets (gdd stacks)
scenario_runner = "pytest-bdd"
```

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `name` | cadena | `?` en `gdd status` | El nombre del proyecto. |
| `description` | cadena | ninguno | Qué es el proyecto, en palabras. Se escribe para los agentes y el Brief; gdd no la lee. |
| `language` | cadena | `en` | El idioma que los agentes usan con la persona. La fija `gdd init --language`. |
| `platform` | cadena | `?` en `gdd status` | Dónde se ejecuta. `gdd init` acepta `mobile`, `desktop`, `web`, `cli` u `other`. |
| `stack` | cadena o array de cadenas | ninguno | Las [fichas de stack](stacks.md) del proyecto, cada una una sola vez. Un `stack` ausente o vacío no nombra ninguna. Cualquier otra cosa (un número, una lista con un número) es un error. |
| `scenario_runner` | cadena | ninguno | El runner de Gherkin. `gdd init` escribe los runners de los stacks, unidos con `, `. Es para los agentes; gdd no la lee. |

## zones.toml

El mapa de zonas: qué rutas tienen qué color. Lo leen `gdd classify`, `gdd new-change`,
`gdd prepare`, las puertas (para clasificar el Change y para la comprobación de alcance),
`gdd status` y `gdd gate zones`. El Orchestrator mapea las zonas rojas al final de Discover; la
persona las aprueba (`gdd gate zones ok`). Consulta [Cómo elige el riesgo las
puertas](../explanation/risk.md).

El color de una ruta es el más alto entre las zonas que la cubren; una ruta que no cubre
ninguna zona toma `default`. El color de un Change es el más alto entre sus rutas, elevado a su
`--min-color` cuando este es más alto.

Patrones de ruta: `**` atraviesa carpetas (`docs/**` cubre todo lo que hay dentro de `docs`;
`**/x` también cubre `x` en la raíz), `*` y `?` se quedan dentro de una carpeta. `*.md` cubre
`README.md` pero no `notes/readme.md`.

### Nivel superior

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `default` | `"green"`, `"yellow"` o `"red"` | `"yellow"` | El color de una ruta que no cubre ninguna zona. `gdd init` escribe `yellow`: código sin zona es código sin tests conocidos. |

### [[zone]]

Una tabla por zona, tantas como quieras, en el orden del fichero. Cuando varias zonas del mismo
color cubren una ruta, la primera da su motivo.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `color` | `"green"`, `"yellow"` o `"red"` | el color de `default` | El color de las rutas. Cualquier otra palabra es un error que la nombra. |
| `paths` | array de cadenas | `[]` | Patrones de ruta, desde la raíz del proyecto. |
| `reason` | cadena | `""` | Por qué estas rutas tienen este color. `gdd classify` y el registro lo muestran. |

`gdd init` escribe tres zonas: `docs/**` y `*.md` en verde; el arnés (`gdd/**`, `.github/**`,
`.claude/**`, `.agents/**`, `.opencode/**`, `.gdd/bin/**`) en rojo; la cadena de suministro de
los stacks (por ejemplo `pyproject.toml`, `uv.lock`, `osv-scanner.toml`,
`osv-scanner-licenses.toml`) en rojo.

La aprobación de las zonas rojas guarda un hash de cada `[[zone]]` cuyo color es `red`, con
todas sus claves. Cualquier edición de una zona roja, incluida una clave añadida, pide una
aprobación nueva.

## rules.toml

La tabla de reglas: qué puertas pide cada color y cada vía, dónde viven los escenarios, los
límites de cada bucle y las rutas que cada rol no debe tocar. Lo leen `gdd classify`,
`gdd new-change`, `gdd prepare`, `gdd g2`, `gdd g3`, `gdd guard`, `gdd scope`, `gdd launch`,
`gdd resume`, `gdd merge`, `gdd release`, `gdd selftest` y `gdd ci`.

### Nivel superior

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `main_branch` | cadena | `"main"` | La rama de la que salen los Changes y en la que se fusionan. `gdd init` escribe la rama en la que estaba el repositorio cuando se ejecutó. |
| `push_main` | booleano | `false` | Hace push de la rama principal tras una fusión, y de las etiquetas tras un release, cuando el repositorio tiene un remoto. |

### [acceptance]

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `dir` | cadena | `"features"` | Carpeta de los ficheros de escenarios (`.feature`). Un `.feature` dentro de ella que un Change crea o cambia necesita la puerta de escenarios (G1, *Scenario Gate*). Su subcarpeta `hidden/` guarda los ejemplos ocultos. |
| `regression` | cadena | `"features/regression"` | Carpeta cuyos ficheros `.feature` no necesitan G1. |
| `required` | booleano | `true` | Cero escenarios en `dir` hacen fallar G3 incluso para un Change sin escenarios propios. Un Change con ficheros de escenarios propios falla G3 con cero escenarios, diga lo que diga esta clave. |

G3 cuenta las líneas de los ficheros `.feature` de `dir` y sus subcarpetas que empiezan por
`Scenario:`, `Scenario Outline:`, `Escenario:` o `Esquema del escenario:`.

### [limits]

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `builder_retries` | entero, 0 o más | `3` | Rondas del modo build contra G2 antes de las replanificaciones. Siempre se ejecuta al menos una ronda. |
| `replans` | entero, 0 o más | `1` | Replanificaciones de un Change que sigue rojo en G2: cada una es una llamada al Orchestrator, otra vez el modo test cuando la replanificación dice `NEXT: test`, una ronda más de build y G2. Después el Change se aparca. |
| `agent_timeout_s` | entero, 0 o más | `3600` | Segundos que puede durar una llamada de agente, hasta que se cierra su salida. Pasado ese tiempo se mata su grupo de procesos y el Change se aparca. `gdd init` no la escribe. |
| `command_seconds` | entero mayor que 0 | `1800` | Segundos que puede durar cada comando de puerta y cada comando de release. Un comando que se queda sin tiempo se mata junto con todo lo que arrancó y termina con el código de salida 124. Cualquier otro valor es un error que nombra `limits.command_seconds`. |
| `reviewer_fixes` | entero | `1` en el fichero que escribe `gdd init` | gdd 0.1.0 no la lee. El Builder recibe siempre una ronda de corrección tras un KO del Reviewer y otra tras un G3 rojo. Cuando un hallazgo del Reviewer nombra un fichero que solo escribe el modo test, esa ronda empieza en modo test. |

### [spike]

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `max_usd` | número mayor que 0 | `5` | Lo que pueden costar las llamadas de agente de un Spike, en USD, antes de que se aparque. Cada variante de un Spike tiene el presupuesto entero para sus propias llamadas. Se comprueba tras cada llamada, a partir del coste que guarda el registro. Cualquier otro valor es un error que nombra `spike.max_usd`. |

### [parallel]

`gdd init` no la escribe.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `max` | entero, 1 o más | `2` | Cuántos Changes pueden construirse a la vez. Un Change cuenta mientras su estado es `building`. `gdd launch` rechaza uno más; un Change en curso que planifica una ruta solapada solo avisa. |
| `folder` | cadena | `"../<project folder>.worktrees"` | La carpeta de los worktrees de los Changes, desde la raíz del proyecto. Cada Change se ejecuta en `<folder>/<id>`. |

### [versions]

`gdd init` no la escribe.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `mode` | `"auto"` o `"manual"` | `"auto"` | `auto`: un Change promovido con una línea de changelog recibe la siguiente versión (etiqueta `v<x.y.z>`) en cuanto su promoción ha pasado. `manual`: las promociones no publican nada, y `gdd release` publica lo que esté fusionado. Cualquier otro valor es un error. |

### [color]

Lo que pide cada color en cada [vía](gates.md#vías), rápida o completa. Un color tiene una
tabla propia, `[color.<color>]`, y una por vía, `[color.<color>.fast]` y
`[color.<color>.full]`. La regla de un color en una vía es su tabla de vía cuando la tiene, y si
no, su tabla de color. Una tabla de vía no hereda de la tabla de color: una clave que omite toma
el valor por defecto de abajo. Un color que el fichero no menciona recibe los valores por
defecto.

Cuando el fichero no tiene ninguna sección `[color]`, gdd usa en su lugar la tabla que lleva
incorporada:

| Color y vía | `tests_first` | `mutation` | `reviewer` | `g4_if_g1` |
|---|---|---|---|---|
| verde, rápida | `false` | `false` | `false` | `false` |
| verde, completa | `true` | `false` | `true` | `true` |
| amarillo, rápida | `true` | `true` | `true` | `false` |
| amarillo, completa | `true` | `true` | `true` | `true` |
| rojo, completa | `true` | `true` | `true` | `true` |

`gdd init` escribe las mismas tablas con `mutation = false` para el amarillo, y `review` en
todas las vías.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `tests_first` | booleano | sin fijar: solo cuando el Change tiene escenarios nuevos (G1) | El Builder en modo test escribe pasos y tests antes del modo build. Un Change verde sin G1 nunca pasa por el modo test, diga lo que diga esta clave. |
| `mutation` | booleano | `false`; rojo: siempre `true` | Se ejecutan los comandos de G2 y G3 cuyo nombre empieza por `mutation`. Si no, aparecen como omitidos. |
| `reviewer` | booleano | `true` | El Reviewer revisa el Change después de G3. |
| `g4_if_g1` | booleano | `true` | Un Change con escenarios nuevos (G1) espera a la persona en la puerta de salida (G4, *Release Gate*). Un Change rojo siempre espera. |
| `review` | `"quick"` o `"deep"` | `"quick"`; rojo: siempre `"deep"` | Con qué profundidad mira el Reviewer. Quick: los ficheros que tocó el Change, su paquete y sus escenarios. Deep: eso, más el código alrededor del Change y lo que puede salir muy mal ahí. |

El mutation testing (pruebas de mutación: pequeños fallos plantados en el código para comprobar
que los tests los detectan) forma parte del núcleo de GDD, que no se puede desactivar para el
rojo. Una tabla roja (con o sin vía) que diga `mutation = false`, `reviewer = false`,
`g4_if_g1 = false` o `review = "quick"` hace que se rechace el fichero entero:

```text
error: gdd/rules.toml: red changes always get mutation testing, the Reviewer and the human G4 (the core of GDD): remove color.red.full.mutation = false from gdd/rules.toml
```

gdd también rechaza una profundidad de revisión distinta de `quick` o `deep` y un color
distinto de `green`, `yellow` o `red`. El rojo nunca va por la vía rápida: `[color.red.fast]`
no se usa nunca.

La vía de un Change es `fast` cuando su color no es rojo, no tiene escenarios nuevos ni
cambiados y se creó con `--resumable`; en otro caso, `full`. Consulta [Vías](gates.md#vías).

### [protected]

Las rutas que cada rol no debe cambiar, como patrones (consulta [zones.toml](#zonestoml)). Path
Guard las comprueba en G2 y G3, antes y después de los comandos, tras cada ronda del modo test
y en `gdd guard`.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `all` | array de cadenas | `[]` | Rutas que ningún rol puede cambiar: el arnés. `gdd ci` hace fallar una rama de Change que cambia alguna. |
| `builder_build` | array de cadenas | `[]` | Rutas que el Builder en modo build no debe cambiar: los escenarios aprobados y los pasos escritos en modo test. |
| `builder_test` | array de cadenas | `[]` | Rutas que el Builder en modo test no debe cambiar: el código de producción. |
| `reviewer` | array de cadenas | `[]` | Rutas que el Reviewer no debe cambiar. `gdd init` escribe `["**"]`: todo. |
| cualquier otra clave | array de cadenas | `[]` | Las rutas del rol con ese nombre, con `-` escrito como `_`. `builder` es el rol con el que juzgan por defecto `gdd g2`, `gdd g3` y `gdd guard`. |

Un rol no debe tocar las rutas de `all` más las de su propia lista. Digan lo que digan estas
listas, cualquier rol falla Path Guard cuando un fichero de escenarios aprobado en G1 ha
cambiado después de la aprobación.

`gdd init` escribe `all` como `gdd/**`, `.github/**`, `.claude/**`, `.agents/**`,
`.opencode/**`, `.gdd/bin/**`, `AGENTS.md` y `CLAUDE.md`, y `builder_build` y `builder_test` a
partir de las [fichas de stack](stacks.md).

## models.toml

Quién hace cada rol: un agente (integración) y su modelo. Lo leen `gdd new-change`,
`gdd classify` y `gdd prepare` (el modelo de cada rol entra en la decisión del Change),
`gdd launch` y `gdd resume` (qué agente arrancar), `gdd status` y `gdd agents`. `gdd init` lo
escribe a partir de `--agents` y de los valores por defecto de cada agente; consulta
[Roles](roles.md#modelos-por-defecto).

```toml
[orchestrator]
integration = "claude"
model = "opus"

[builder.green]
integration = "claude"
model = "haiku"

[builder.yellow]
integration = "claude"
model = "sonnet"

[builder.red]
integration = "claude"
model = "opus"

[builder_test]
integration = "claude"
model = "sonnet"

[reviewer]
integration = "codex"
model = "gpt-6-astra"
effort = "high"

[families]
"claude" = "anthropic"
"codex" = "openai"
```

### [orchestrator], [builder.\<color\>], [builder_test], [reviewer]

El modelo de cada rol. `[builder.<color>]` es el Builder en modo build para los Changes de ese
color (`green`, `yellow`, `red`). `[builder_test]` es el Builder en modo test para todos los
colores. `[orchestrator]` replanifica un Change atascado en G2. `[reviewer]` revisa cada Change
que tiene Reviewer. Un rol sin tabla no tiene agente, y su llamada falla.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `integration` | cadena | ninguno | El agente: `claude`, `codex` u `opencode` (su ficha de agente). |
| `model` | cadena | ninguno | El modelo, tal como lo nombra el agente. Se pasa al `--model` del agente (`-m` en OpenCode). |
| `effort` | cadena | ninguno | Esfuerzo de razonamiento, para los agentes cuya ficha lo admite: Codex recibe `-c model_reasoning_effort=<effort>` (`low`, `medium`, `high`, `xhigh`). Claude Code y OpenCode lo ignoran. |
| `same_family_accepted` | cadena, `YYYY-MM-DD` | ninguno | Solo en `[reviewer]`: el día en que la persona aceptó un Reviewer de la misma familia de modelos que el Builder. La escribe `gdd init --accept-same-family`. Sin ella, `gdd prepare` rechaza un Change así. |

Las demás claves de una tabla de rol se guardan, tal cual, en la decisión del Change.

### [families]

A qué familia de modelos pertenece cada modelo. El Reviewer debe ser de una familia distinta de
la del Builder, para no compartir sus puntos ciegos.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| cualquier clave | cadena | ninguno | `"<model name prefix or integration>" = "<family>"`. |

La familia de un modelo es la de la primera clave, en el orden del fichero, por la que empieza
el nombre de su modelo o con la que coincide su integración; si no hay ninguna, la propia
integración. `gdd init` escribe una clave por agente: `claude` es `anthropic`, `codex` es
`openai`, `opencode` es `minimax`.

### [session]

`gdd init` no la escribe.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `builder` | `"separate"` o `"shared"` | `"separate"` | `separate`: cada llamada al Builder empieza una sesión propia; el modo build lee lo que escribió el modo test. `shared`: cada llamada al Builder de una ejecución continúa la sesión de la llamada al Builder anterior, en cualquiera de los dos modos. Cualquier otro valor hace que se rechace el fichero. |

Un Change recibe una sesión compartida solo cuando `[builder_test]` y su `[builder.<color>]`
usan la misma integración y la ficha de ese agente dice cómo reanudar una sesión (hoy, Claude
Code). Si no, se ejecuta con sesiones separadas y `gdd status` dice por qué. El Reviewer y el
Orchestrator nunca se unen a la sesión compartida. `gdd launch` y `gdd resume` leen esta tabla
desde el checkout principal.

### [env]

`gdd init` no la escribe.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `pass` | array de cadenas | `[]` | Nombres exactos de variables que los agentes reciben aunque su nombre o su valor indique que guardan un secreto. Consulta [Variables de entorno](environment.md#qué-variables-guardan-un-secreto). |

### [prices.\<model\>]

`gdd init` no la escribe. El precio de un modelo, para los agentes que no informan de su coste
(Codex): una tabla por nombre de modelo, tal como lo escribe `model`.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `input` | número | `0` | USD por millón de tokens de entrada no leídos de la caché. |
| `cache` | número | `0` | USD por millón de tokens de entrada leídos de la caché. |
| `output` | número | `0` | USD por millón de tokens de salida. |

La estimación va a la línea de la llamada en el log. El registro del Change, y por tanto el
presupuesto del Spike, guarda solo lo que informó el agente.

## commands.toml

Los comandos que ejecutan las máquinas: los comandos de las puertas, las herramientas de Learn,
cómo contar la deuda conocida, el test que falla del autotest y los pasos de release. Cada
comando es una línea de shell que se ejecuta con `sh -c` desde la raíz del proyecto, sin nada
en su entrada estándar. Pasa con el código de salida 0. `gdd init` escribe este fichero a
partir de las [fichas de stack](stacks.md) (lo que imprime `gdd stacks compose`) seguidas de la
sección `[release]`. El texto que escribe un agente nunca llega a un shell: solo llegan estos
comandos.

### Nivel superior

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `install` | cadena | ninguno | Los comandos de instalación de cada stack, unidos con ` && `. gdd no lo ejecuta: `gdd bootstrap` ejecuta la primera instalación propia de cada ficha, y el workflow de CI tiene sus propios pasos de instalación. |

### [g2]

Los comandos de la puerta del bucle (G2, *Loop Gate*), que se ejecutan en cada ronda del
Builder y con `gdd g2`.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| cualquier nombre | cadena | ninguno | Un comando. Los comandos se ejecutan en el orden alfabético de sus nombres. |

- Un valor vacío, o que no es una cadena, no está configurado: nunca se ejecuta y no se lista.
- Una puerta sin ningún comando configurado falla (`G2 not configured`).
- Un comando cuyo nombre empieza por `mutation` solo se ejecuta cuando el Change necesita
  mutation testing (pruebas de mutación); si no, aparece en `skipped`.
- Cada comando recibe `GDD_CHANGE`, `GDD_COLOR`, `GDD_DIFF_FILES` y `GDD_FEATURES`; consulta
  [Variables de entorno](environment.md#qué-reciben-los-comandos-de-las-puertas).
- Cuando un comando falla, el informe de la puerta conserva las 40 primeras líneas de su
  salida que informan de un fallo (empiezan por `error` o contienen `FAILED`, `Failed`,
  `failed`, `panicked`, `Error:`, `Traceback` o `✘`), y después los últimos 2500 caracteres.

Las fichas de stack llaman a los comandos de G2 `lint`, `types`, `unit`, `acceptance` y
`architecture`.

### [g3]

Los comandos de la puerta de integración (G3, *Merge Gate*), que ejecuta la receta después de
G2 y también `gdd g3`. Mismas reglas que [`[g2]`](#g2). Las fichas de stack los llaman
`acceptance_full`, `lockfile`, `security`, `dependencies`, `secrets` y `mutation`. `gdd status`
avisa cuando ningún comando de `[g3]` empieza por `mutation`, porque entonces los Changes rojos
se quedarían sin mutation testing.

`gdd adopt` ejecuta una vez cada comando de G2 y G3, salvo aquellos cuyo nombre empieza por
`acceptance`, `mutation` o `secrets`.

### [learn]

Las herramientas que `gdd learn` ejecuta una vez sobre el proyecto. Solo avisan; nunca son una
puerta.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| cualquier nombre | cadena | ninguno | Una herramienta. Las herramientas se ejecutan en el orden del fichero. Su nombre solo admite letras, dígitos, `-` y `_`: lo que dice se guarda en `.gdd/learn/<name>.txt`. Los valores vacíos y los que no son cadenas se descartan. |

Las fichas de stack las llaman `duplication` y `unused`.

### [count]

Cómo contar lo que informa un comando con deuda conocida, para que su deuda tenga un techo que
solo puede bajar (un trinquete).

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| un nombre de comando | array de cadenas | ninguno | Textos que buscar en la salida del comando. Ninguno está vacío y cada uno lleva `{n}` como mucho una vez. Un texto con `{n}` suma el número de cada sitio donde coincide; un texto sin él cuenta las líneas que lo contienen. Cualquier otra cosa es un error que nombra el comando. |

Un comando con deuda conocida que falla pasa la puerta mientras su recuento se mantenga igual o
por debajo del recuento de [`gdd/debt.toml`](#debttoml). Informar de más, o un recuento que gdd
no puede leer, hace fallar la puerta.

### [selftest]

El test que falla a propósito en el autotest del arnés (`gdd selftest`, comprobación 1).

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `file` | cadena | ninguno | Dónde va el test que falla, desde la raíz del proyecto. |
| `content` | cadena | ninguno | El test que falla. |

### [protected]

`gdd init` también escribe aquí la tabla `[protected]` de las fichas de stack compuestas. gdd
no la lee de este fichero: las listas que usa Path Guard están en
[`gdd/rules.toml`](#protected).

### [release]

Los pasos de release. Los leen `gdd merge`, `gdd release` y `gdd selftest`. Cada paso ejecuta
su comando y nada más, dentro de `[limits] command_seconds`, con `GDD_CHANGE` y, cuando
el Change ya tiene versión, `GDD_VERSION`.

| Clave | Tipo | Por defecto | Significado |
|---|---|---|---|
| `simulated` | booleano | `false` | `true`: un paso sin comando pasa y el log dice `simulated`. `false`: un paso sin comando falla. `gdd init` escribe `true`. |
| `build` | cadena | `""` | Construye el artefacto una sola vez. Se ejecuta en la fusión. |
| `deploy_pre` | cadena | `""` | Despliega el artefacto en pre. Se ejecuta en la fusión, después de `build`. |
| `tests_pre` | cadena | `""` | Tests en pre. `gdd release pre`. |
| `demo` | cadena | `""` | La demo para G4: vídeo, capturas de pantalla o la ejecución legible de los escenarios, que se deja en `.gdd/changes/$GDD_CHANGE/demo/`. `gdd release pre`, después de `tests_pre`. |
| `promote` | cadena | `""` | Promueve el mismo artefacto de pre a producción. `gdd release promote`. |
| `observe` | cadena | `""` | Falla cuando suben los errores en producción. `gdd release observe`. |
| `rollback` | cadena | `""` | Vuelve a la versión anterior de producción. Se ejecuta cuando falla `observe`. |
| `rollback_pre` | cadena | `""` | Revierte pre. `gdd selftest` lo ejecuta para la comprobación 3; sin él, la comprobación 3 solo pasa cuando `simulated = true`. |

Un valor vacío cuenta como si no hubiera comando.

## release.toml

Un proyecto no tiene `gdd/release.toml`. Los pasos de release están en la tabla
[`[release]`](#release) de `gdd/commands.toml`: `gdd init` los añade ahí a partir de su
plantilla.

## generated.toml

Lo que `gdd init` escribió una vez y lo que se le pidió, para que `gdd upgrade` pueda generar
los mismos ficheros con un gdd más reciente y fusionarlos con las ediciones del proyecto. Lo
escribe `gdd init`; lo reescribe `gdd upgrade` en su rama `gdd-upgrade`. No lo edites a mano.

### Nivel superior

| Clave | Tipo | Significado |
|---|---|---|
| `version` | cadena | El gdd que escribió los ficheros. |
| `files` | array de cadenas | Los ficheros que `gdd init` escribe una vez: `gdd/project.toml`, `gdd/commands.toml`, `gdd/rules.toml`, `gdd/zones.toml`, `gdd/models.toml`, `docs/brief.md`, `.github/workflows/gdd.yml`. |

### [answers]

| Clave | Tipo | Significado |
|---|---|---|
| `name` | cadena | `--name`. |
| `description` | cadena | `--description`; vacía si no se dio. |
| `platform` | cadena | `--platform`. |
| `stacks` | array de cadenas | `--stack`, cada uno una sola vez. |
| `agents` | array de cadenas | `--agents`, o los agentes encontrados en el `PATH`; el primero construye. |
| `language` | cadena | `--language`. |
| `sensitive` | array de cadenas | `--sensitive`, un área en cada elemento. |
| `accept_same_family` | booleano | `--accept-same-family`. |
| `day` | cadena | El día en que se ejecutó `gdd init`, `YYYY-MM-DD`. |
| `main` | cadena | La rama principal que escribió en los ficheros. |

`gdd upgrade` rechaza un proyecto sin este fichero o sin `[answers]`.

## debt.toml

La deuda conocida de un proyecto que ya tenía código: los comandos de puerta que fallaban
cuando se ejecutó `gdd adopt`. Cada uno avisa en lugar de hacer fallar su puerta hasta que un
Change consigue que pase. `gdd adopt` escribe el fichero, hace un commit solo con él y se niega
a ejecutarse una vez que existe. Después, una fusión elimina la deuda que sus puertas saldaron
y rebaja los recuentos que sus puertas informaron más bajos.

```toml
adopted = "2026-10-30"

[[debt]]
command = "lint"
gate = "g2"
since = "2026-10-30"
count = 3
```

### Nivel superior

| Clave | Tipo | Significado |
|---|---|---|
| `adopted` | cadena | El día en que `gdd adopt` la registró. |

### [[debt]]

| Clave | Tipo | Significado |
|---|---|---|
| `command` | cadena | El nombre del comando en `gdd/commands.toml`. |
| `gate` | `"g2"` o `"g3"` | La sección que lo contiene. |
| `since` | cadena | El día en que se registró. |
| `count` | entero, opcional | Lo que informó el comando, contado con [`[count]`](#count): su techo. Sin esta clave, el comando solo avisa mientras falla. |

Un fichero de deuda que gdd no puede leer hace fallar la comprobación `debt` de la puerta, y la
primera comprobación de `gdd selftest`, que somete G2 a la misma deuda.

## Ver también

- [Puertas](gates.md): qué ejecuta cada puerta con estos ajustes, por color.
- [Stacks](stacks.md): de dónde salen los comandos y las rutas protegidas de `gdd init`.
- [Ajustar las puertas](../how-to/gates-and-commands.md): cambia comandos y límites paso a
  paso.
- [`gdd init`](cli/init.md): los flags que rellenan estos ficheros.
