# Stacks

Las fichas de stack que trae gdd: Python, Rust y TypeScript. Una ficha da a un proyecto su
ejecutor de escenarios, los comandos de las puertas, las herramientas de Learn, el autotest,
las rutas protegidas, la preparación de CI y un esqueleto para un proyecto nuevo. Esta página
recoge cada valor de cada ficha tal como lo trae gdd 0.1.0.

## Usar las fichas

Un proyecto nombra sus fichas, una o varias, en `stack` de
[`gdd/project.toml`](configuration.md#projecttoml). `gdd init --stack` escribe esa lista y
rellena el proyecto a partir de las fichas:

| De la ficha | Va a |
|---|---|
| `[g2]`, `[g3]`, `[learn]`, `[count]`, `[selftest]`, `install` | `gdd/commands.toml` |
| `builder_build`, `builder_test` | `[protected]` de `gdd/rules.toml` (y una copia en `gdd/commands.toml`) |
| Ficheros de la cadena de suministro | Una zona roja de `gdd/zones.toml` |
| Rutas ignoradas | `.gitignore`, junto con `.gdd/` |
| Preparación de CI | Los pasos de `.github/workflows/gdd.yml` antes de `bin/gdd ci` |
| Esqueleto | Los ficheros de un proyecto nuevo, cuando la carpeta no tiene ninguno de los ficheros por los que se detecta la ficha |
| Instrucciones para agentes | El comando `gdd-stack-<name>` de cada agente (`gdd agents install`) |

```bash
gdd stacks
gdd stacks show python
gdd stacks compose python typescript
```

Para `gdd stacks`, deberías ver:

```text
python · Python · pytest-bdd
rust · Rust · cucumber-rs
typescript · TypeScript · cucumber-js
```

`gdd stacks`, `show` y `compose` solo se ejecutan dentro de un proyecto GDD. `compose` no
escribe nada: imprime la forma de `gdd/commands.toml` más una tabla `[protected]` para las
fichas indicadas.

`gdd bootstrap` ejecuta la primera instalación de cada stack (`bootstrap`, en las fichas de
abajo), en orden, se detiene en la primera que falla y hace commit de los ficheros de la cadena
de suministro que creó o cambió. `gdd init` no instala nada.

### Varios stacks

`gdd stacks compose` y `gdd init` combinan las fichas así:

- `install`: las instalaciones de todas las fichas, unidas con ` && `.
- Un nombre de comando que tiene el mismo comando en todas las fichas donde no está vacío se
  ejecuta una sola vez, con su nombre (`secrets`, `duplication`).
- Si no, un comando por ficha, llamado `<name>-<stack>` (`lint-python`, `lint-typescript`).
  Los nombres conservan su comienzo, así que `mutation-python` sigue ejecutándose solo cuando
  toca mutation testing.
- Los comandos vacíos se omiten. Los textos de `[count]` siguen el nombre de su comando.
- `[selftest]` sale de la primera ficha; las rutas protegidas y los ficheros de la cadena de
  suministro de todas las fichas se suman, cada uno una sola vez.
- Dos esqueletos que escriben la misma ruta con distinto texto son un error.

### Escenarios y ejemplos ocultos

Todos los ejecutores son estrictos: una frase sin paso, o un paso pendiente, falla. G2 pasa los
ficheros de escenarios del Change en `GDD_FEATURES`, una ruta por línea; G3 los ejecuta todos
con los ejemplos ocultos (consulta [Variables de
entorno](environment.md#variables-que-leen-los-ejecutores-de-escenarios)).

| Stack | G2 ejecuta | Ejemplos ocultos |
|---|---|---|
| Python | `GDD_FEATURES`, todo `features/` si está vacía | Los ficheros bajo `features/hidden/` solo se ejecutan con `GDD_HIDDEN=1` (G3) |
| Rust | `GDD_FEATURES`, todo `features/` si está vacía | Los ficheros bajo `features/hidden/` solo se ejecutan con `GDD_HIDDEN=1` (G3) |
| TypeScript | `GDD_FEATURES`, todo `features/` si está vacía, sin los escenarios etiquetados `@hidden` | G3 ejecuta todos los `features/**/*.feature`, incluidos los `@hidden` |

## python

Python con uv. Código en `src/<package>/`, tests unitarios en `tests/unit/`, pasos de
escenario en `tests/acceptance/`.

| Elemento | Valor |
|---|---|
| Ejecutor de escenarios | pytest-bdd |
| Se detecta por | `pyproject.toml` |
| Cadena de suministro (zona roja) | `pyproject.toml`, `uv.lock`, `osv-scanner.toml`, `osv-scanner-licenses.toml` |
| `install` | `uv sync --locked` |
| Primera instalación (`gdd bootstrap`) | `uv lock && uv sync --locked` |
| `.gitignore` | `.venv/`, `__pycache__/`, `.mypy_cache/`, `.pytest_cache/`, `.ruff_cache/`, `mutants/` |
| Fichero del autotest | `tests/unit/test_gdd_selftest.py` |
| `builder_build` | `features/**`, `tests/acceptance/**` |
| `builder_test` | `src/**` |
| Excepciones de seguridad | `[[IgnoredVulns]]` en `osv-scanner.toml`, cada una con su motivo |

### Comandos de puerta y de Learn

```toml
[g2]
lint = "uv run ruff check . && uv run ruff format --check ."
types = "uv run mypy src"
unit = "uv run pytest -q tests/unit"
acceptance = "uv run pytest -q tests/acceptance || [ $? -eq 5 ]"
architecture = "uv run lint-imports"

[g3]
acceptance_full = "GDD_FEATURES= GDD_HIDDEN=1 uv run pytest -q tests/acceptance || [ $? -eq 5 ]"
lockfile = "uv lock --check"
security = "uv run bandit -q -r src && osv-scanner scan source -L uv.lock"
dependencies = """R=$(mktemp) && trap 'rm -f "$R"' EXIT && uv export --frozen --no-dev --no-emit-project --format requirements-txt -q -o "$R" && { ! grep -q '^[A-Za-z0-9]' "$R" || osv-scanner scan source --config=osv-scanner-licenses.toml --licenses="MIT,Apache-2.0,BSD-2-Clause,BSD-3-Clause,ISC,0BSD,Unicode-3.0,Zlib,MPL-2.0,PSF-2.0" -L "requirements.txt:$R"; }"""
secrets = "BASE=$(git rev-parse -q --verify main || git rev-parse origin/main) && git add -A --intent-to-add && git diff $(git merge-base HEAD $BASE) | gitleaks stdin --no-banner --redact"
mutation = '''set -f; P=$(printf '%s\n' "$GDD_DIFF_FILES" | sed -n 's#^src/\(.*\)\.py$#\1#p' | tr / . | sed 's/$/*/'); [ -z "$P" ] || { uv run mutmut run $P && ! uv run mutmut results | grep -q survived; }'''

[learn]
duplication = "npx --yes jscpd --silent src"
unused = "uv run vulture src"

[count]
lint = ["Found {n} error", "{n} file would be reformatted", "{n} files would be reformatted"]
types = ["Found {n} error"]
unit = ["{n} failed", "{n} error"]
```

- `acceptance`: el código de salida 5 (aún no se ha recogido ningún escenario) pasa; G3 cuenta
  los escenarios por su cuenta.
- `dependencies`: solo las licencias de las dependencias de ejecución; si aún no hay ninguna,
  pasa. Las exporta a un fichero temporal, que se borra al salir, y no escribe nada en el
  proyecto.
- `mutation`: mutmut sobre los módulos de `src/` que tocó el Change; cualquier superviviente
  hace fallar el comando.
- Complejidad: la regla `C901` de Ruff (`max-complexity = 10` en el `pyproject.toml` del
  esqueleto).

### Herramientas

| Necesidad | Dónde | Herramienta | Alternativa |
|---|---|---|---|
| Lint y formato | G2 | Ruff | |
| Tipos | G2 | mypy | Pyright |
| Tests unitarios | G2 | pytest | |
| Ejecutor de Gherkin | G2, G3 | pytest-bdd | behave |
| Regla de arquitectura | G2 | import-linter | |
| Seguridad | G3 | Bandit + OSV-Scanner | pip-audit |
| Licencias | G3 | Lista de licencias permitidas de OSV-Scanner | pip-licenses |
| Mutación | G3 | mutmut | |
| Secretos | G3 | gitleaks, sobre el diff del Change | TruffleHog |
| Duplicación (aviso) | Learn | jscpd | |
| Código sin usar | Learn | vulture | |
| UI y demo | pre | Playwright para Python | |

`gdd stacks show python` imprime el mismo catálogo con las cifras de comunidad que recoge la
ficha.

### Qué instalar

uv (el esqueleto pide Python 3.12 o posterior), OSV-Scanner, gitleaks y Node.js para
`npx jscpd`. Las herramientas de Python llegan con `uv sync`: el grupo de desarrollo del
esqueleto trae bandit, import-linter, mutmut, mypy, pytest, pytest-bdd, ruff y vulture. pytest
se queda por debajo de 9.2 (`pytest>=9.1.1,<9.2`), porque pytest 9.2 rompe pytest-bdd 9.0.0
(issue 850 de pytest-bdd).

Preparación de CI: `astral-sh/setup-uv@v7`, `uv sync --locked` y OSV-Scanner 2.7.0
verificado contra sus checksums.

### Esqueleto

| Fichero | Contiene |
|---|---|
| `pyproject.toml` | El paquete, el grupo de desarrollo y la configuración de Ruff, mypy (strict), pytest (`bdd_features_base_dir = "features/"`), import-linter (sus contratos, en `[tool.importlinter]`) y mutmut |
| `osv-scanner.toml`, `osv-scanner-licenses.toml` | La configuración de OSV-Scanner para `security` y `dependencies` |
| `src/<package>/__init__.py`, `src/<package>/greeting.py` | Un primer módulo |
| `tests/__init__.py`, `tests/unit/__init__.py`, `tests/unit/test_greeting.py` | Un primer test unitario |
| `tests/acceptance/__init__.py`, `tests/acceptance/steps/__init__.py` | Las definiciones de pasos, un módulo por feature |
| `tests/acceptance/test_features.py` | Enlaza las features con los pasos mediante `scenarios()`, a partir de `GDD_FEATURES` y `GDD_HIDDEN` |
| `features/.gitkeep` | La carpeta de escenarios |

`<package>` es el nombre del proyecto en minúsculas, con cada tramo de caracteres distintos de
`a-z` y `0-9` convertido en un solo `_` y sin `_` en los extremos (`app_` delante de un dígito
inicial, `app` cuando no queda nada).

## rust

Rust con cargo. Tests unitarios junto al código y pasos de escenario en `tests/acceptance/`,
que ejecuta un target de test llamado `acceptance` con `harness = false` y `test = false`: un
`cargo test` a secas lo salta y `cargo test --test acceptance` lo ejecuta.

| Elemento | Valor |
|---|---|
| Ejecutor de escenarios | cucumber-rs |
| Se detecta por | `Cargo.toml` |
| Cadena de suministro (zona roja) | `Cargo.lock`, `deny.toml`, `rust-toolchain.toml` |
| `install` | `cargo fetch --locked` |
| Primera instalación (`gdd bootstrap`) | `cargo generate-lockfile` |
| `.gitignore` | `/target` |
| Fichero del autotest | `tests/gdd_selftest_failing.rs` |
| `builder_build` | `features/**`, `tests/acceptance/**`, `tests/acceptance.rs` |
| `builder_test` | `src/**`, `crates/*/src/**` |
| Excepciones de seguridad | `[advisories] ignore` en `deny.toml`, cada una con su motivo |

### Comandos de puerta y de Learn

```toml
[g2]
lint = "cargo fmt --all --check && cargo clippy --all-targets --test acceptance -- -D warnings -W clippy::cognitive_complexity"
types = ""
unit = "cargo test --workspace"
acceptance = "cargo test --test acceptance"
architecture = ""

[g3]
acceptance_full = "GDD_FEATURES= GDD_HIDDEN=1 cargo test --test acceptance --locked"
lockfile = "cargo metadata --locked --format-version 1 > /dev/null"
security = "cargo deny --locked check advisories bans sources"
dependencies = "cargo deny --locked check licenses"
secrets = "BASE=$(git rev-parse -q --verify main || git rev-parse origin/main) && git add -A --intent-to-add && git diff $(git merge-base HEAD $BASE) | gitleaks stdin --no-banner --redact"
mutation = """export CARGO_PROFILE_TEST_DEBUG=0; if command -v prlimit >/dev/null; then export CARGO_TARGET_$(rustc -vV | sed -n 's/^host: //p' | tr a-z- A-Z_)_RUNNER='prlimit --as=2147483648 --' MALLOC_ARENA_MAX=2; fi; if command -v mold >/dev/null && [ -z "${RUSTFLAGS:-}" ]; then export RUSTFLAGS='-C link-arg=-fuse-ld=mold'; fi; BASE=$(git rev-parse -q --verify main || git rev-parse origin/main) && mkdir -p target && git add -A --intent-to-add && git diff $(git merge-base HEAD $BASE) > target/gdd-change.diff && { missed_too() { if [ $code -eq 3 ] && [ -s target/mutants.out/missed.txt ]; then echo "$(wc -l < target/mutants.out/missed.txt | tr -d ' ') mutant(s) missed besides the timeouts"; code=2; fi; }; cargo mutants --in-diff target/gdd-change.diff --output target --no-shuffle -j 2; code=$?; missed_too; if [ $code -eq 3 ]; then cargo mutants --in-diff target/gdd-change.diff --output target --no-shuffle --iterate -j 1 --minimum-test-timeout 120; code=$?; missed_too; if [ $code -eq 3 ]; then if [ -f target/mutants.out/timeout.txt ]; then n=$(wc -l < target/mutants.out/timeout.txt | tr -d ' '); else n=some; fi; echo "$n mutant(s) hung again: counted as caught"; code=0; fi; fi; exit $code; }"""

[learn]
duplication = "npx --yes jscpd --silent src"
unused = "cargo machete"

[count]
lint = ["Diff in ", "due to {n} previous error"]
unit = ["{n} failed;"]
```

- `types` está vacío: clippy comprueba los tipos de todos los targets. `architecture` está
  vacío: escribe una regla con `cargo tree` entre los crates del proyecto.
- `mutation`: cargo-mutants sobre el diff del Change, compilado sin información de
  depuración, enlazado con mold cuando está instalado y `RUSTFLAGS` no está definida, y con
  cada proceso de test limitado a 2 GB donde existe `prlimit`. Un mutante no detectado hace
  fallar el comando. Una ejecución en la que los mutantes solo agotaron el tiempo tiene una
  segunda oportunidad, de uno en uno y con al menos 120 s por test; un mutante que vuelve a
  colgarse cuenta como detectado, y el informe de la puerta conserva la última línea del
  comando.
- Complejidad: `cognitive_complexity` de clippy, como aviso que `-D warnings` convierte en
  error.

### Herramientas

| Necesidad | Dónde | Herramienta | Alternativa |
|---|---|---|---|
| Lint y formato | G2 | rustfmt + clippy | |
| Tests unitarios | G2 | cargo test | cargo-nextest |
| Ejecutor de Gherkin | G2, G3 | cucumber-rs | |
| Seguridad y licencias | G3 | cargo-deny | cargo-audit (solo vulnerabilidades) |
| Mutación | G3 | cargo-mutants | |
| Secretos | G3 | gitleaks, sobre el diff del Change | TruffleHog |
| Complejidad | G2 | `cognitive_complexity` de clippy | |
| Duplicación (aviso) | Learn | jscpd | |
| Dependencias sin usar | Learn | cargo-machete | |

### Qué instalar

Un toolchain estable de Rust con rustfmt y clippy, cargo-deny, cargo-mutants, cargo-machete,
gitleaks y Node.js para `npx jscpd`. Opcional: mold (mutantes más rápidos) y `prlimit` (el
límite de memoria, en Linux).

Preparación de CI: `dtolnay/rust-toolchain@stable` con rustfmt y clippy, y
`taiki-e/install-action@v2` con cargo-deny y cargo-mutants.

### Esqueleto

| Fichero | Contiene |
|---|---|
| `Cargo.toml` | El paquete (edición 2024, `publish = false`), cucumber y futures como dependencias de desarrollo, y el target de test `acceptance` |
| `deny.toml` | cargo-deny: avisos de seguridad, licencias permitidas, prohibiciones y orígenes |
| `src/lib.rs` | Una primera función y sus tests unitarios |
| `tests/acceptance.rs` | El ejecutor de cucumber-rs, que filtra por `GDD_FEATURES` y `GDD_HIDDEN` |
| `tests/acceptance/world.rs` | El World de los pasos |
| `features/.gitkeep` | La carpeta de escenarios |

## typescript

TypeScript con npm. Código en `src/`, tests unitarios en `tests/unit/` (`*.test.ts`), pasos de
escenario en `features/steps/`.

| Elemento | Valor |
|---|---|
| Ejecutor de escenarios | cucumber-js |
| Se detecta por | `package.json`, `tsconfig.json` |
| Cadena de suministro (zona roja) | `package-lock.json`, `package.json`, `.npmrc`, `osv-scanner.toml`, `osv-scanner-licenses.toml` |
| `install` | `npm ci` |
| Primera instalación (`gdd bootstrap`) | `npm install --no-fund --no-audit` |
| `.gitignore` | `node_modules/`, `reports/`, `coverage/`, `.stryker-tmp/` |
| Fichero del autotest | `tests/unit/gdd-selftest.test.ts` |
| `builder_build` | `features/**` |
| `builder_test` | `src/**` |
| Excepciones de seguridad | `[[IgnoredVulns]]` en `osv-scanner.toml`, cada una con su motivo |

### Comandos de puerta y de Learn

```toml
[g2]
lint = "npx eslint . --max-warnings 0"
types = "npx tsc --noEmit"
unit = "npx vitest run"
acceptance = '''npx cucumber-js --strict --tags 'not @hidden' $(printf '%s' "$GDD_FEATURES" | tr '\n' ' ')'''
architecture = "npx depcruise src --config .dependency-cruiser.cjs"

[g3]
acceptance_full = "npx cucumber-js --strict 'features/**/*.feature' "
lockfile = "npm ci --ignore-scripts --no-audit --no-fund"
security = "osv-scanner scan source -L package-lock.json"
dependencies = 'osv-scanner scan source --config=osv-scanner-licenses.toml --licenses="MIT,Apache-2.0,BSD-2-Clause,BSD-3-Clause,ISC,0BSD,Unicode-3.0,Zlib,MPL-2.0" -L package-lock.json'
secrets = "BASE=$(git rev-parse -q --verify main || git rev-parse origin/main) && git add -A --intent-to-add && git diff $(git merge-base HEAD $BASE) | gitleaks stdin --no-banner --redact"
mutation = '''F=$(printf '%s\n' "$GDD_DIFF_FILES" | grep -E '^src/.*\.ts$' | paste -sd, -); [ -z "$F" ] || npx stryker run --mutate "$F"'''

[learn]
duplication = "npx --yes jscpd --silent src"
unused = "npx knip"

[count]
lint = ["✖ {n} problem"]
types = ["error TS"]
unit = ["Tests  {n} failed"]
```

- `security`: incluye las herramientas de desarrollo. `dependencies`: las licencias de lo que
  se distribuye.
- `mutation`: StrykerJS sobre los ficheros `src/*.ts` que tocó el Change; el
  `stryker.config.json` del esqueleto rompe con cualquier superviviente
  (`thresholds.break = 100`).
- Complejidad: la regla `complexity` de ESLint (10 en el esqueleto), con typescript-eslint y
  eslint-plugin-security.

### Herramientas

| Necesidad | Dónde | Herramienta | Alternativa |
|---|---|---|---|
| Lint y formato | G2 | ESLint + typescript-eslint | Biome |
| Tipos | G2 | tsc | |
| Tests unitarios | G2 | Vitest | Jest |
| Ejecutor de Gherkin | G2, G3 | cucumber-js | playwright-bdd, para escenarios `@ui` |
| Regla de arquitectura | G2 | dependency-cruiser | |
| Seguridad y licencias | G3 | OSV-Scanner + eslint-plugin-security | npm audit (solo vulnerabilidades) |
| Mutación | G3 | StrykerJS | |
| Secretos | G3 | gitleaks, sobre el diff del Change | TruffleHog |
| Duplicación (aviso) | Learn | jscpd | |
| Código sin usar | Learn | knip | |
| UI y demo | pre | Playwright | |

### Qué instalar

Node.js y npm (CI usa Node 22), OSV-Scanner y gitleaks. El resto llega con `npm ci`: las
dependencias de desarrollo del esqueleto son @cucumber/cucumber, @eslint/js,
@stryker-mutator/core, @stryker-mutator/vitest-runner, @types/node, dependency-cruiser, eslint,
eslint-plugin-security, tsx, typescript, typescript-eslint y vitest. jscpd y knip se ejecutan
mediante `npx`.

Preparación de CI: `actions/setup-node@v6` con Node 22, `npm ci` y OSV-Scanner 2.7.0
verificado contra sus checksums.

### Esqueleto

| Fichero | Contiene |
|---|---|
| `package.json` | Un paquete privado de módulos ES con las dependencias de desarrollo |
| `tsconfig.json`, `vitest.config.ts` | La configuración de TypeScript y de Vitest |
| `eslint.config.js` | typescript-eslint (strict), el plugin de seguridad, `complexity` 10 |
| `.dependency-cruiser.cjs` | Las reglas de arquitectura: sin imports circulares |
| `stryker.config.json` | StrykerJS con el runner de Vitest, todos los umbrales a 100 |
| `cucumber.js` | cucumber-js, en modo estricto, con los pasos en `features/steps/**/*.ts` mediante tsx |
| `features/support/tsx-register.js`, `features/steps/.gitkeep` | La carga de los pasos en TypeScript; la carpeta de pasos |
| `src/greeting.ts`, `tests/unit/greeting.test.ts` | Un primer módulo y su test unitario |
| `osv-scanner.toml`, `osv-scanner-licenses.toml` | La configuración de OSV-Scanner para `security` y `dependencies` |

## Todos los stacks

Sean cuales sean los stacks, el workflow de CI que escribe `gdd init` hace checkout del
repositorio con todo su historial, ejecuta la preparación de cada ficha, instala gitleaks
8.30.1 (verificado contra sus checksums) y el propio gdd (`cargo install --locked` desde el
repositorio git de gdd, en la etiqueta de la versión que escribió el workflow), y después
ejecuta `bin/gdd ci`.

El comando `secrets` y el comando `mutation` de Rust toman como rama principal `main`, o
`origin/main` cuando no hay un `main` local.

## Ver también

- [commands.toml](configuration.md#commandstoml): qué hace gdd con cada comando.
- [Puertas](gates.md): cuándo se ejecuta cada comando.
- [Instalar gdd](../how-to/install.md): el binario, los agentes y las herramientas que usan
  las puertas.
- [Referencia de comandos](cli.md#gdd-stacks): `gdd stacks`, `show` y `compose`.
