# Instalar gdd

Instala el binario `gdd`, al menos un agente de programación que pueda manejar y las
herramientas que ejecutan las puertas de tu stack. Al final compruebas que las tres cosas están
en su sitio.

## Antes de empezar

- Linux o macOS. gdd ejecuta sus puertas con `sh` y los agentes en worktrees de git.
- [git](https://git-scm.com).
- [Rust](https://rustup.rs) 1.98 o posterior, para compilar gdd. `rustup update` pone al día un
  toolchain más antiguo.

gdd 0.1.0 solo se instala con cargo. Los binarios precompilados, crates.io y Homebrew están en
la [hoja de ruta](../../../ROADMAP.md#5-open-source) y todavía no están disponibles.

## 1. Instala el binario

Instala la versión etiquetada desde GitHub. Es el mismo comando que ejecuta el workflow de CI
que escribe [`gdd init`](../reference/cli.md#gdd-init):

```bash
cargo install --locked --git https://github.com/MarsGotta/gdd --tag v0.1.0 gdd
```

cargo compila gdd y lo deja en `~/.cargo/bin`, que rustup añade a tu `PATH`.

> [!NOTE]
> El workflow que escribe `gdd init` instala gdd desde la etiqueta `v<version>`, así que la CI
> funciona en cuanto esa etiqueta existe en GitHub. Si cargo dice `failed to find tag v0.1.0`,
> la etiqueta aún no está publicada: instala desde la rama main con el mismo comando, sin
> `--tag v0.1.0`.

### O compílalo desde el código fuente

Clona el repositorio e instala desde esa copia:

```bash
git clone https://github.com/MarsGotta/gdd
cd gdd
cargo install --locked --path crates/gdd
```

El repositorio fija su toolchain en `rust-toolchain.toml`, así que rustup descarga Rust 1.98.1
la primera vez que compilas en él. Para trabajar en el propio gdd, sigue
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

En un proyecto, gdd siempre se llama a través de `bin/gdd`, un script corto que escribe
`gdd init`. Ese script ejecuta el `gdd` de tu `PATH`, o el binario que indique la variable
`GDD_BIN`:

```bash
GDD_BIN=/home/me/src/gdd/target/release/gdd bin/gdd status
```

## 2. Instala un agente de programación

gdd no habla con ningún modelo por sí mismo. Arranca el programa de línea de comandos de cada
agente de programación, en modo headless (sin interfaz), con los argumentos del rol que
desempeña. Conoce tres agentes, cada uno con su ficha de datos:

| Agente | Programa en el `PATH` | Comandos escritos en | Se llaman con |
|---|---|---|---|
| [Claude Code](https://code.claude.com/docs) | `claude` | `.claude/skills/` | `/gdd-status` |
| [Codex CLI](https://developers.openai.com/codex) | `codex` | `.agents/skills/` | `$gdd-status` |
| [OpenCode](https://opencode.ai/docs) | `opencode` | `.opencode/commands/` | `/gdd-status` |

Instala los que quieras siguiendo sus propias instrucciones e inicia sesión en cada uno una vez,
de forma interactiva. gdd usa esas sesiones (sirve una suscripción). Nunca entrega tus claves de
API a un agente: quita del entorno del agente toda variable cuyo nombre contenga una palabra
como `KEY`, `TOKEN`, `SECRET` o `PASSWORD`, y toda URL que lleve una contraseña, salvo que
`[env] pass` en `gdd/models.toml` nombre esa variable a propósito.
[Variables de entorno](../reference/environment.md#qué-variables-guardan-un-secreto) tiene la
regla completa.

Instala agentes de **dos familias de modelos**. El Reviewer tiene que ser de una familia
distinta a la del Builder, para que no comparta sus puntos ciegos: por ejemplo, Claude Code para
construir y Codex para revisar. Con una sola familia, `gdd init` se niega y te dice qué hacer:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

`gdd init --agents` indica los agentes de un proyecto; el primero es el que construye. Sin esa
opción, init toma los que encuentra en el `PATH`. [Elegir agentes y modelos](agents-and-models.md)
explica cómo cambiarlos más adelante.

## 3. Instala las herramientas de tu stack

Cada ficha de stack indica los comandos que ejecutan sus puertas. Algunas herramientas llegan
con las dependencias del propio proyecto, que instala
[`gdd bootstrap`](../reference/cli.md#gdd-bootstrap); las demás tienen que estar en tu `PATH`.

### Python

| Para qué | Herramienta | Cómo se instala |
|---|---|---|
| Entorno, todos los comandos | [uv](https://docs.astral.sh/uv/) | la instalas tú |
| Lint, tipos, tests unitarios, escenarios, arquitectura, seguridad, mutación, código sin usar | Ruff, mypy, pytest, pytest-bdd, import-linter, Bandit, mutmut, vulture | dependencias de desarrollo en `pyproject.toml`, que instala `gdd bootstrap` |
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

El mutation testing en Rust enlaza con [mold](https://github.com/rui314/mold) cuando está
instalado, y limita cada proceso de test con `prlimit` donde existe (Linux). Los dos son
opcionales.

### TypeScript

| Para qué | Herramienta | Cómo se instala |
|---|---|---|
| Todos los comandos | Node.js y npm | los instalas tú |
| Lint, tipos, tests unitarios, escenarios, arquitectura, mutación | ESLint, tsc, Vitest, cucumber-js, dependency-cruiser, StrykerJS | dependencias de desarrollo en `package.json`, que instala `gdd bootstrap` |
| Seguridad y licencias de G3 | OSV-Scanner | la instalas tú |
| Secretos de G3 | gitleaks | la instalas tú |
| Learn: código sin usar, duplicación | knip, jscpd, a través de `npx` | las descarga `npx` |

`gdd stacks show NAME` imprime la ficha completa de un stack, con todos los comandos que
ejecutan sus puertas. La [referencia de stacks](../reference/stacks.md) las recoge todas.

## 4. Comprueba la instalación

Las comprobaciones que leen una ficha de stack se ejecutan dentro de un proyecto GDD. Crea uno
de usar y tirar:

```bash
mkdir notes && cd notes
git init -b main
gdd init --name notes --platform cli --stack python --agents claude codex
bin/gdd bootstrap
```

`gdd init` lista todos los ficheros que ha escrito y hace commit de ellos; `gdd bootstrap`
instala las dependencias del stack y hace commit del fichero de bloqueo (*lock file*). Después
comprueba cada pieza:

```bash
bin/gdd stacks
bin/gdd agents check
bin/gdd selftest
```

Deberías ver:

```text
python · Python · pytest-bdd
rust · Rust · cucumber-rs
typescript · TypeScript · cucumber-js
{"ok":true}
OK a failing test turns G2 red
OK editing a protected file is blocked (Path Guard and CI)
OK rollback works in pre (simulated)
OK zero scenarios counts as a failure
harness 4/4
```

- `gdd stacks` lista las fichas de stack que trae gdd.
- `gdd agents check` confirma que los comandos de GDD de la carpeta de cada agente son los que
  escribe gdd.
- `gdd selftest` demuestra que el arnés funciona: un test que falla pone G2 en rojo, Path Guard
  bloquea un fichero protegido, el rollback funciona en pre y cero escenarios hacen fallar G3.
  Cualquier resultado que no llegue a `harness 4/4` nombra la comprobación que ha fallado.

El autotest no ejecuta todos los comandos de las puertas. El primer Change que lances es la
prueba real de las herramientas del paso 3: si falta alguna, aparece como una puerta fallida en
su Decision Inbox.

## Siguientes pasos

- [Tu primer Change](../getting-started/tutorial.md): prepara un proyecto y lleva un Change
  desde el escenario hasta la publicación.
- [Llevar gdd a un proyecto existente](adopt-existing-project.md): cuando la carpeta ya tiene
  código.
- [Elegir agentes y modelos](agents-and-models.md): qué agente y qué modelo desempeña cada rol.
