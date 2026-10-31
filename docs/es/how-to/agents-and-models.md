# Elegir agentes y modelos

Usa esta guía cuando quieras cambiar qué agente de programación o qué modelo hace un rol, añadir
un agente a un proyecto o poner precio a las llamadas de un agente que no informa de su coste.

## Antes de empezar

- Un proyecto GDD: consulta [Llevar gdd a un proyecto existente](adopt-existing-project.md) o
  [Tu primer Change](../getting-started/tutorial.md).
- Los agentes que quieres usar están instalados y con la sesión iniciada: Claude Code
  (`claude`), Codex (`codex`) u OpenCode (`opencode`). Consulta [Instalar gdd](install.md).
- Los roles se describen en [Roles](../reference/roles.md).

`gdd/models.toml` guarda todas las decisiones de esta página. Forma parte del
[arnés](../glossary.md#arnés), los ficheros que gdd usa para comprobar tu proyecto, así que
ningún agente puede editarlo. Cámbialo tú, en la rama main, entre un Change y otro.

## 1. Mira quién hace cada rol

[`gdd init`](../reference/cli/init.md) rellena `gdd/models.toml` a partir de los agentes que
elegiste en el formulario (o indicaste con `--agents`) o, si no, de los que tienen su programa en
el `PATH`. El primer agente orquesta y construye. El Reviewer es el primer agente de otra
familia de modelos. Con `--agents claude codex`:

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

El [color](../glossary.md#color) de un Change elige el modelo del Builder en modo build: un
modelo ligero para el trabajo verde, uno estándar para el amarillo y el más potente para el
rojo. El modo test, el Reviewer y el Orchestrator tienen un solo modelo cada uno, sea cual sea
el color:

```mermaid
flowchart TD
    C{Color del Change} -->|verde| G[builder.green]:::green
    C -->|amarillo| Y[builder.yellow]:::yellow
    C -->|rojo| R[builder.red]:::red
    T[builder_test: modo test] --> B[Build]
    G --> B
    Y --> B
    R --> B
    B --> V[reviewer: otra familia]
    classDef green fill:#d9f2e3,stroke:#2e8b57,color:#123
    classDef yellow fill:#fff4cc,stroke:#c9a400,color:#321
    classDef red fill:#fbdcdc,stroke:#c0392b,color:#311
```

Para ver qué recibirá un Change concreto, clasifícalo con
[`gdd classify`](../reference/cli/classify.md):

```bash
gdd classify CHANGE
```

La salida JSON (*JavaScript Object Notation*) nombra la tabla de cada rol (`builder`,
`builder_test`, `reviewer_model`, `orchestrator`) y el ajuste `builder_session` del paso 4.

## 2. Cambia un agente o un modelo

Para cambiar quién hace un rol, edita la tabla del rol. `integration` es el agente (`claude`,
`codex` u `opencode`). `model` es un nombre que ese agente acepte: un alias como `sonnet` en
Claude Code, un id de modelo en Codex, `provider/model` en OpenCode. Por ejemplo, un Builder más
barato para el trabajo verde a través de OpenCode:

```toml
[builder.green]
integration = "opencode"
model = "minimax/MiniMax-M3"
```

Si el nombre del modelo tiene un prefijo nuevo, o el agente es nuevo, añade su familia a
`[families]` (paso 3). Si el agente es nuevo, instala también sus comandos (paso 5).

Si usas Codex, `effort` fija su esfuerzo de razonamiento (`low`, `medium`, `high`, `xhigh`), que
gdd pasa como `-c model_reasoning_effort=<effort>`. Los demás agentes ignoran la clave.

## 3. Mantén al Reviewer en otra familia de modelos

El Reviewer tiene que ser de una [familia de modelos](../glossary.md#familia-de-modelos)
distinta a la del Builder, para que no pase por alto los mismos errores;
[Mantener honestos a los agentes](../explanation/safety.md#capa-2-jueces-que-el-builder-no-puede-tocar)
explica por qué. gdd busca la familia de un modelo en `[families]`: la primera clave por la que
empieza el nombre de su modelo, o que coincide con su integración.

```toml
[families]
"claude" = "anthropic"
"codex" = "openai"
"minimax" = "minimax"
```

[`gdd prepare`](../reference/cli/prepare.md), el primer paso de cada lanzamiento, rechaza un
Change cuyo Reviewer sea de la familia del Builder:

```text
error: Reviewer and Builder are the same model family: edit gdd/models.toml.
```

Si solo tienes agentes de una familia, acéptalo una vez, a sabiendas, al ejecutar init:

```bash
gdd init --name notes --platform cli --stack python --agents claude --accept-same-family
```

Sin la opción, init se detiene:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

Con ella, la tabla del Reviewer registra el día en que lo aceptaste:

```toml
[reviewer]
integration = "claude"
model = "opus"
same_family_accepted = "2026-10-30"   # the human accepted one model family only
```

En un proyecto existente, añade a mano la misma línea `same_family_accepted = "YYYY-MM-DD"`.
Cuando tengas disponible una segunda familia, sustituye al Reviewer.
[`gdd status`](../reference/cli/status.md) te avisa cuando encuentra una en la máquina:

```text
A second model family is on this machine (codex, opencode): set it as the Reviewer in gdd/models.toml
```

## 4. Comparte una sesión del Builder (opcional)

Por defecto, el modo test y el modo build se ejecutan en sesiones separadas: el modo build
empieza de cero y lee lo que escribió el modo test. Si quieres que el modo build continúe la
conversación del modo test, y que cada ronda posterior la continúe también, pon:

```toml
[session]
builder = "shared"   # or "separate", the default
```

Path Guard, el mutation testing y el Reviewer no cambian. El Reviewer nunca se une a la sesión
del Builder.

Una sesión compartida solo funciona si el modo test y el modo build usan el mismo agente y la
ficha de ese agente dice cómo reanudar una sesión, algo que hoy solo cumple Claude Code. Si no,
las sesiones siguen separadas y `gdd status` explica por qué:

```text
Builder session: shared is set, but test mode (claude) and build mode (codex) use different agents: the sessions stay separate
```

Cualquier otro valor detiene todos los comandos que leen el fichero:

```text
error: gdd/models.toml: invalid TOML: session.builder must be separate or shared, not "one"
```

> [!NOTE]
> Medido en el propio repositorio de gdd (40 ejecuciones): la sesión compartida usó un 21 %
> menos de tokens nuevos y un 7 % menos de tiempo, con una calidad dentro del ruido. Por eso es
> una opción y no el valor por defecto.
> Consulta [features/builder-session.feature](../../../features/builder-session.feature).

## 5. Instala los comandos de GDD en cada agente

Cada agente encuentra los comandos de GDD (`gdd-setup`, `gdd-change`, `gdd-build`,
`gdd-review`...) y las instrucciones de cada stack (`gdd-stack-<name>`) en su propia carpeta.
`gdd init` los escribió una vez. Después de editar `gdd/models.toml` o de actualizar gdd, vuelve
a escribirlos con [`gdd agents`](../reference/cli/agents.md):

```bash
gdd agents install
```

Sin `--agent`, escribe para cada `integration` de `gdd/models.toml`. Imprime las rutas como JSON
y no escribe ningún otro fichero, así que tus propias skills y tus comandos se quedan como
están.

| Agente | Ficheros que recibe | Cómo llamas a un comando |
|---|---|---|
| Claude Code | `.claude/skills/gdd-<command>/SKILL.md` | `/gdd-status` |
| Codex | `.agents/skills/gdd-<command>/SKILL.md` | `$gdd-status` |
| OpenCode | `.opencode/commands/gdd-<command>.md` | `/gdd-status` |

Para añadir un agente que el fichero de modelos todavía no usa:

```bash
gdd agents install --agent opencode
```

```text
{"ok":true,"written":[".opencode/commands/gdd-build.md",".opencode/commands/gdd-build-test.md",...,".opencode/commands/gdd-stack-python.md"]}
```

`install` no hace commit. Estas carpetas son arnés, en una zona roja: revísalas y haz commit de
ellas tú, en main.

Para comprobar que no se ha editado ni perdido nada:

```bash
gdd agents check
```

```text
error: .claude/skills/gdd-report/SKILL.md: missing
error: .claude/skills/gdd-status/SKILL.md: differs from what gdd writes
```

Sale con 1 y nombra cada fichero. Después de `install` imprime `{"ok":true}`. Cada fichero
avisa al principio de que está generado, así que lo que edites ahí se pierde.

Las fichas de agente (dónde guarda cada agente sus comandos, cómo lo lanza gdd, qué puede hacer
cada rol) vienen integradas en gdd. Un agente sin ficha se rechaza (`no agent sheet`).

Permitan lo que permitan los permisos propios del agente, un agente que haya arrancado gdd solo
puede ejecutar `gdd g2`, `gdd guard`, `gdd note`, `gdd status`, `gdd classify`, `gdd inbox`,
`gdd report` y `gdd scope`. gdd rechaza cualquier otro comando que venga de él. Consulta
[Qué puede ejecutar un agente](../reference/roles.md#qué-puede-ejecutar-un-agente).

## 6. Rellena los precios para las estimaciones

gdd registra los tokens y el coste de cada llamada de agente. Cada agente informa de ellos a su
manera:

| Agente | Tokens | Coste |
|---|---|---|
| Claude Code | sí | sí, su propia estimación en el cliente |
| Codex | sí | no |
| OpenCode | sí | sí (no informa del modelo: el registro guarda el que pidió gdd) |

Si un agente no informa del coste, añade los precios de su modelo en `[prices.<model>]`, en
dólares estadounidenses (USD) por millón de tokens, y gdd calcula el coste:

```toml
[prices.gpt-6-astra]
input = 0.0    # per million input tokens not read from the cache
cache = 0.0    # per million input tokens read from the cache
output = 0.0   # per million output tokens
```

Copia las cifras de la página de precios del proveedor; nunca te las inventes. Sin precios, esas
llamadas registran 0 USD. Si pagas una suscripción, el coste es solo una estimación, así que
compara tokens. Consulta [Medir el trabajo](measure.md).

## 7. Otros ajustes de los agentes

Secretos: los agentes nunca reciben una variable cuyo nombre contenga una palabra de secreto
(`KEY`, `TOKEN`, `SECRET`, `PASS`, `PASSWORD`, `CREDENTIALS`, `DSN` y unas pocas más) ni una URL
con una contraseña como valor. Si un agente necesita una, nómbrala en `[env] pass = ["NAME"]` de
`gdd/models.toml`.
[Variables de entorno](../reference/environment.md#qué-variables-guardan-un-secreto) tiene la
regla completa.

Tiempo límite: una llamada de agente puede durar `agent_timeout_s` segundos, 3600 por defecto.
Si las llamadas necesitan más tiempo, súbelo en `[limits]` de `gdd/rules.toml`.

Todas las claves están en [Configuración](../reference/configuration.md#modelstoml).

## Siguientes pasos

- [Ajustar las puertas](gates-and-commands.md): lo que comprueban las máquinas alrededor de los
  agentes.
- [Medir el trabajo](measure.md): tokens y coste por rol, modelo y agente.
- [Mantener honestos a los agentes](../explanation/safety.md): por qué el Reviewer es de otra
  familia y qué puede tocar cada rol.
