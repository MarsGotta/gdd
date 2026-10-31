# Instalar gdd

Sigue esta guía una vez en cada máquina en la que tú o tus agentes vayáis a usar GDD. Instalas
el binario `gdd`, al menos un agente de programación y las herramientas que ejecutan las
puertas de tu stack, y después preparas un proyecto con `gdd init`.

## Antes de empezar

- Linux o macOS. gdd ejecuta sus puertas con `sh` y los agentes en worktrees de git.
- [git](https://git-scm.com), con tu nombre y tu correo configurados
  (`git config --global user.name` y `user.email`): gdd hace commit de lo que escribe.
- [Rust](https://rustup.rs) 1.98 o posterior, para compilar gdd. Si tu toolchain es más
  antiguo, `rustup update` lo pone al día.
- Las herramientas de tu stack. Las [puertas](../glossary.md#puerta) de gdd ejecutan programas
  reales (tu linter, tus tests, un escáner de seguridad), así que una puerta falla cuando falta
  su programa. Estas las instalas tú; el resto llega con las dependencias del proyecto (el paso
  3 tiene el detalle):

| Stack | Las instalas tú |
|---|---|
| Python | uv, OSV-Scanner, gitleaks; Node.js para una comprobación de Learn |
| Rust | cargo-deny, cargo-mutants, gitleaks; cargo-machete y Node.js para Learn |
| TypeScript | Node.js y npm, OSV-Scanner, gitleaks |

La [referencia de stacks](../reference/stacks.md) recoge todos los comandos que ejecutan las
puertas de cada stack.

gdd 0.1.0 solo se instala con cargo. Los binarios precompilados, crates.io y Homebrew están en
la [hoja de ruta](../../../ROADMAP.md#5-open-source) y todavía no están disponibles.

## 1. Instala el binario

Instala la versión etiquetada desde GitHub. El workflow de CI (integración continua) que
escribe `gdd init` ejecuta este mismo comando:

```bash
cargo install --locked --git https://github.com/MarsGotta/gdd --tag v0.1.0 gdd
```

cargo compila gdd y lo deja en `~/.cargo/bin`, que rustup añade a tu `PATH`. Tú, tus agentes y
CI ejecutáis este mismo `gdd`.

> [!NOTE]
> El workflow que escribe `gdd init` instala gdd desde la etiqueta `v<version>`, así que la CI
> funciona en cuanto esa etiqueta existe en GitHub. Si cargo dice `failed to find tag v0.1.0`,
> la etiqueta aún no está publicada: instala desde la rama main con el mismo comando, sin
> `--tag v0.1.0`.

### O compílalo desde el código fuente

Si prefieres una copia local, clona el repositorio e instala desde esa copia:

```bash
git clone https://github.com/MarsGotta/gdd
cd gdd
cargo install --locked --path crates/gdd
```

El repositorio fija su toolchain en `rust-toolchain.toml`, así que rustup descarga Rust 1.98.1
la primera vez que compilas en él. Si quieres trabajar en el propio gdd, sigue
[CONTRIBUTING.md](../../../CONTRIBUTING.md) en su lugar: compila el binario que usan las puertas
del propio proyecto.

### Comprueba el binario

```bash
gdd --version
```

Deberías ver:

```text
gdd 0.1.0
```

## 2. Instala un agente de programación

gdd arranca el programa de línea de comandos de cada agente de programación, en modo headless
(sin ventana), con el rol que le asigna. Tú hablas con esos mismos agentes a través de las
skills de GDD, como `/gdd-change`. gdd conoce tres agentes, cada uno con su ficha de datos:

| Agente | Programa en el `PATH` | Skills escritas en | Se llaman con |
|---|---|---|---|
| [Claude Code](https://code.claude.com/docs) | `claude` | `.claude/skills/` | `/gdd-status` |
| [Codex CLI](https://developers.openai.com/codex) | `codex` | `.agents/skills/` | `$gdd-status` |
| [OpenCode](https://opencode.ai/docs) | `opencode` | `.opencode/commands/` | `/gdd-status` |

Instala los que quieras siguiendo sus propias instrucciones e inicia sesión en cada uno una vez,
de forma interactiva. gdd usa esas sesiones, así que sirve una suscripción. gdd nunca entrega
tus claves de API (interfaz de programación de aplicaciones) a un agente:
[Variables de entorno](../reference/environment.md#qué-variables-guardan-un-secreto) tiene la
regla.

Si puedes, instala agentes de **dos familias de modelos**: por ejemplo, Claude Code para
construir y Codex para revisar. El Reviewer tiene que ser de una familia distinta a la del
Builder, para que no pase por alto los mismos errores;
[Mantener honestos a los agentes](../explanation/safety.md#capa-2-jueces-que-el-builder-no-puede-tocar)
explica por qué. Si solo tienes una familia, el formulario de Setup te pide un sí explícito, y
`gdd init` con opciones se niega y te dice qué hacer:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

Para cambiar de agentes o de modelos más adelante, consulta
[Elegir agentes y modelos](agents-and-models.md).

## 3. Instala las herramientas de tu stack

Cada [ficha de stack](../glossary.md#ficha-de-stack) indica los comandos que ejecutan sus
puertas. Algunas herramientas son dependencias de desarrollo del proyecto: las instala el
formulario de Setup de `gdd init` cuando ejecuta la primera instalación de cada stack. Las demás
las instalas tú, en tu `PATH`. En las tablas, G3 es la puerta de integración (*Merge Gate*), las
comprobaciones que pasa un Change antes de fusionarse, y Learn es la
[fase](../glossary.md#learn) que convierte los problemas repetidos en reglas.

### Python

| Para qué | Herramienta | Cómo se instala |
|---|---|---|
| Entorno, todos los comandos | [uv](https://docs.astral.sh/uv/) | la instalas tú |
| Lint, tipos, tests unitarios, escenarios, arquitectura, seguridad, mutación, código sin usar | Ruff, mypy, pytest, pytest-bdd, import-linter, Bandit, mutmut, vulture | dependencias de desarrollo en `pyproject.toml`; las instala el formulario de Setup |
| Seguridad y licencias de G3 | [OSV-Scanner](https://google.github.io/osv-scanner/) | la instalas tú |
| Secretos de G3 | [gitleaks](https://github.com/gitleaks/gitleaks) | la instalas tú |
| Learn: duplicación (solo avisa) | jscpd, a través de `npx` | necesita [Node.js](https://nodejs.org) |

### Rust

| Para qué | Herramienta | Cómo se instala |
|---|---|---|
| Lint, formato, tests unitarios, escenarios | rustfmt, clippy, cargo test, cucumber-rs | componentes de rustup; cucumber es una dependencia de desarrollo |
| Seguridad, orígenes y licencias de G3 | [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | `cargo install --locked cargo-deny` |
| Mutation testing de G3 | [cargo-mutants](https://mutants.rs) | `cargo install --locked cargo-mutants` |
| Secretos de G3 | gitleaks | la instalas tú |
| Learn: dependencias sin usar, duplicación | cargo-machete, jscpd a través de `npx` | `cargo install --locked cargo-machete`; Node.js |

Dos herramientas son opcionales en Rust. Si [mold](https://github.com/rui314/mold) está
instalado, el mutation testing enlaza con él. Si existe `prlimit` (Linux), limita cada proceso
de test.

### TypeScript

| Para qué | Herramienta | Cómo se instala |
|---|---|---|
| Todos los comandos | Node.js y npm | los instalas tú |
| Lint, tipos, tests unitarios, escenarios, arquitectura, mutación | ESLint, tsc, Vitest, cucumber-js, dependency-cruiser, StrykerJS | dependencias de desarrollo en `package.json`; las instala el formulario de Setup |
| Seguridad y licencias de G3 | OSV-Scanner | la instalas tú |
| Secretos de G3 | gitleaks | la instalas tú |
| Learn: código sin usar, duplicación | knip, jscpd, a través de `npx` | las descarga `npx` |

La [referencia de stacks](../reference/stacks.md) recoge todos los comandos que ejecuta cada
ficha.

## 4. Prepara un proyecto

1. Abre un terminal en una carpeta nueva, o en la raíz de un repositorio que quieras llevar a
   GDD.
2. Ejecuta `gdd init`:

```bash
gdd init
```

3. Responde al formulario de Setup, un paso cada vez. Deberías ver:

<!-- Real screens of gdd 0.1.0 at 100 columns, blank lines trimmed. -->
```text
┌ gdd init · Step 1 of 7 · Project name ───────────────────────────────────────────────────────────┐
│What is the project called?                                                                       │
│> █                                                                                               │
│Enter: next · Ctrl-C: quit                                                                        │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

El formulario pregunta el nombre, una descripción, la plataforma, los stacks (o `Not sure`), los
agentes de programación, el idioma en que los agentes te hablan y qué sería grave si fallara.
Los agentes que encuentra en tu máquina salen marcados, y el primero marcado es el que
construye:

```text
┌ gdd init · Step 5 of 7 · Coding agents ──────────────────────────────────────────────────────────┐
│Which coding agents? The first checked builds; those found on this machine come checked.          │
│> [x] claude                                                                                      │
│  [x] codex                                                                                       │
│  [x] opencode                                                                                    │
│Up/Down: move · Space: check · Enter: next · Esc: back · Ctrl-C: quit                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Si quieres cambiar una respuesta, pulsa `Esc`: vuelve un paso atrás y conserva lo que
escribiste. No se escribe nada hasta el resumen:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Name: notes                                                                                       │
│Description: A command-line notes tool                                                            │
│Platform: cli                                                                                     │
│Stacks: rust                                                                                      │
│Agents: claude, codex, opencode                                                                   │
│Language: en                                                                                      │
│Sensitive: Losing notes                                                                           │
│Enter to set up · Esc to go back                                                                  │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` escribe el [arnés](../glossary.md#arnés), los ficheros que gdd usa para comprobar tu
proyecto, y hace commit (`gdd init 0.1.0`). Después, el formulario ejecuta la primera
instalación de cada stack, que escribe el fichero de bloqueo (*lock file*, el que fija la
versión exacta de cada dependencia), y gdd también hace commit de él (`gdd bootstrap`). gdd
lista todos los ficheros que ha escrito y lo que imprimió la instalación. En un proyecto
Python, deberías ver:

```text
...
.agents/skills/gdd-stack-python/SKILL.md
python: uv lock && uv sync --locked
...
Committed: uv.lock
Next: start your coding agent here and run the gdd-setup command (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

La última línea es la que importa: a partir de aquí, trabajas a través de tu agente. El
[inicio rápido](../getting-started/quickstart.md) sigue desde este punto.

Si la carpeta ya tiene código, sigue en su lugar
[Llevar gdd a un proyecto existente](adopt-existing-project.md).

Si la instalación falla (sin red, falta una herramienta), el arnés se queda en su commit y gdd
dice qué ha fallado. Arregla la causa y ejecuta
[`gdd bootstrap`](../reference/cli/bootstrap.md) para repetir la instalación. Consulta
[La primera instalación falló](troubleshooting.md#falló-la-primera-instalación).

> [!NOTE]
> Si elegiste `Not sure` para la tecnología, el formulario no prepara el proyecto. Termina con
> el comando `gdd init` que debes ejecutar cuando lo sepas, y te dice que tu agente de
> programación puede ayudarte a elegir. Todavía no hay ninguna skill de GDD instalada, así que
> pregúntale a tu agente con tus propias palabras.

Una herramienta del paso 3 que falte puede pasar desapercibida hasta el primer Change que
lances: el autotest que ejecuta `/gdd-setup` no ejecuta todos los comandos de las puertas. Si
falta alguna, aparece como una puerta fallida en la Decision Inbox de ese Change.

## Por dentro

El formulario da las mismas respuestas que las opciones de [`gdd init`](../reference/cli/init.md),
que un script o CI usa en su lugar. [`gdd bootstrap`](../reference/cli/bootstrap.md) ejecuta la
primera instalación de cada stack (después de `gdd init` con opciones, ejecútala tú o deja que
la ejecute `/gdd-setup`), [`gdd agents`](../reference/cli/agents.md) escribe y comprueba las
skills de la carpeta de cada agente, y [`gdd stacks`](../reference/cli/stacks.md) lista las
fichas de stack que trae gdd.

## Siguientes pasos

- [Inicio rápido](../getting-started/quickstart.md): de `gdd init` a tu primer Change
  publicado, hablando con tu agente.
- [Llevar gdd a un proyecto existente](adopt-existing-project.md): cuando la carpeta ya tiene
  código.
- [Elegir agentes y modelos](agents-and-models.md): qué agente y qué modelo desempeña cada rol.
