# Stacks

The stack sheets built into gdd: Python, Rust and TypeScript. A sheet gives a project its
scenario runner, gate commands, Learn tools, self-test, protected paths, CI setup and a
skeleton for a new project. This page lists every value of each sheet as gdd 0.1.0 ships it.

## Using sheets

A project names its sheets in `stack` of [`gdd/project.toml`](configuration.md#projecttoml),
one or more. `gdd init --stack` writes that list and fills the project from the sheets:

| From the sheet | Goes to |
|---|---|
| `[g2]`, `[g3]`, `[learn]`, `[count]`, `[selftest]`, `install` | `gdd/commands.toml` |
| `builder_build`, `builder_test` | `[protected]` of `gdd/rules.toml` (and a copy in `gdd/commands.toml`) |
| Supply chain files | A red zone of `gdd/zones.toml` |
| Ignored paths | `.gitignore`, with `.gdd/` |
| CI setup | The steps of `.github/workflows/gdd.yml` before `bin/gdd ci` |
| Skeleton | The files of a new project, when the folder has none of the sheet's detect files |
| Instructions for agents | The `gdd-stack-<name>` command of each agent (`gdd agents install`) |

```bash
gdd stacks
gdd stacks show python
gdd stacks compose python typescript
```

You should see, for `gdd stacks`:

```text
python · Python · pytest-bdd
rust · Rust · cucumber-rs
typescript · TypeScript · cucumber-js
```

`gdd stacks`, `show` and `compose` run inside a GDD project only. `compose` writes nothing:
it prints the shape of `gdd/commands.toml` plus a `[protected]` table for the sheets named.

`gdd bootstrap` runs the first install (`bootstrap` below) of each stack, in order, stops at
the first that fails, and commits the supply chain files it created or changed. `gdd init`
installs nothing.

### Several stacks

`gdd stacks compose` and `gdd init` combine sheets this way:

- `install`: every sheet's install, joined with ` && `.
- A command name that has the same command in every sheet where it is not empty runs once,
  under its name (`secrets`, `duplication`).
- Otherwise one command per sheet, named `<name>-<stack>` (`lint-python`, `lint-typescript`).
  Names keep their start, so `mutation-python` still runs only when mutation testing is due.
- Empty commands are left out. `[count]` texts follow their command's name.
- `[selftest]` comes from the first sheet; the protected paths and supply chain files of all
  sheets add up, each once.
- Two skeletons that write the same path with different text are an error.

### Scenarios and hidden examples

Every runner is strict: a sentence with no step, or a pending step, fails. G2 gives the
change's scenario files in `GDD_FEATURES`, one path per line; G3 runs all of them with the
hidden examples (see [Environment
variables](environment.md#variables-the-scenario-runners-read)).

| Stack | G2 runs | Hidden examples |
|---|---|---|
| Python | `GDD_FEATURES`, all of `features/` when empty | Files under `features/hidden/` run only with `GDD_HIDDEN=1` (G3) |
| Rust | `GDD_FEATURES`, all of `features/` when empty | Files under `features/hidden/` run only with `GDD_HIDDEN=1` (G3) |
| TypeScript | `GDD_FEATURES`, all of `features/` when empty, without scenarios tagged `@hidden` | G3 runs every `features/**/*.feature`, `@hidden` included |

## python

Python with uv. Code in `src/<package>/`, unit tests in `tests/unit/`, scenario steps in
`tests/acceptance/`.

| Item | Value |
|---|---|
| Scenario runner | pytest-bdd |
| Detected by | `pyproject.toml` |
| Supply chain (red zone) | `pyproject.toml`, `uv.lock`, `osv-scanner.toml`, `osv-scanner-licenses.toml` |
| `install` | `uv sync --locked` |
| First install (`gdd bootstrap`) | `uv lock && uv sync --locked` |
| `.gitignore` | `.venv/`, `__pycache__/`, `.mypy_cache/`, `.pytest_cache/`, `.ruff_cache/`, `mutants/` |
| Self-test file | `tests/unit/test_gdd_selftest.py` |
| `builder_build` | `features/**`, `tests/acceptance/**` |
| `builder_test` | `src/**` |
| Security exceptions | `[[IgnoredVulns]]` in `osv-scanner.toml`, each with a reason |

### Gate and Learn commands

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

- `acceptance`: exit code 5 (no scenario collected yet) passes; G3 counts the scenarios itself.
- `dependencies`: the licenses of the runtime dependencies only; none yet is a pass. It exports
  them to a temporary file, removed on exit, and writes nothing in the project.
- `mutation`: mutmut on the modules of `src/` the change touched; any survivor fails.
- Complexity: Ruff's `C901` (`max-complexity = 10` in the skeleton's `pyproject.toml`).

### Tools

| Need | Where | Tool | Alternative |
|---|---|---|---|
| Lint and format | G2 | Ruff | |
| Types | G2 | mypy | Pyright |
| Unit tests | G2 | pytest | |
| Gherkin runner | G2, G3 | pytest-bdd | behave |
| Architecture rule | G2 | import-linter | |
| Security | G3 | Bandit + OSV-Scanner | pip-audit |
| Licenses | G3 | OSV-Scanner license allowlist | pip-licenses |
| Mutation | G3 | mutmut | |
| Secrets | G3 | gitleaks, on the change's diff | TruffleHog |
| Duplication (warning) | Learn | jscpd | |
| Unused code | Learn | vulture | |
| UI and demo | pre | Playwright for Python | |

`gdd stacks show python` prints the same catalog with the community figures the sheet records.

### To install

uv (the skeleton asks for Python 3.12 or later), OSV-Scanner, gitleaks, and Node.js for
`npx jscpd`. The Python tools come with `uv sync`: the skeleton's development group has
bandit, import-linter, mutmut, mypy, pytest, pytest-bdd, ruff and vulture. pytest stays below
9.2 (`pytest>=9.1.1,<9.2`), because pytest 9.2 breaks pytest-bdd 9.0.0 (pytest-bdd issue 850).

CI setup: `astral-sh/setup-uv@v7`, `uv sync --locked`, and OSV-Scanner 2.7.0 checked against
its checksums.

### Skeleton

| File | Holds |
|---|---|
| `pyproject.toml` | The package, the development group, and the settings of Ruff, mypy (strict), pytest (`bdd_features_base_dir = "features/"`), import-linter (its contracts, in `[tool.importlinter]`) and mutmut |
| `osv-scanner.toml`, `osv-scanner-licenses.toml` | OSV-Scanner settings for `security` and `dependencies` |
| `src/<package>/__init__.py`, `src/<package>/greeting.py` | A first module |
| `tests/__init__.py`, `tests/unit/__init__.py`, `tests/unit/test_greeting.py` | A first unit test |
| `tests/acceptance/__init__.py`, `tests/acceptance/steps/__init__.py` | The step definitions, one module per feature |
| `tests/acceptance/test_features.py` | Binds the features to the steps with `scenarios()`, from `GDD_FEATURES` and `GDD_HIDDEN` |
| `features/.gitkeep` | The scenario folder |

`<package>` is the project name in lowercase, each run of other characters than `a-z` and `0-9`
turned into one `_`, without `_` at either end (`app_` before a leading digit, `app` when
nothing is left).

## rust

Rust with cargo. Unit tests next to the code, scenario steps in `tests/acceptance/`, run by a
test target named `acceptance` with `harness = false` and `test = false`: plain `cargo test`
skips it, `cargo test --test acceptance` runs it.

| Item | Value |
|---|---|
| Scenario runner | cucumber-rs |
| Detected by | `Cargo.toml` |
| Supply chain (red zone) | `Cargo.lock`, `deny.toml`, `rust-toolchain.toml` |
| `install` | `cargo fetch --locked` |
| First install (`gdd bootstrap`) | `cargo generate-lockfile` |
| `.gitignore` | `/target` |
| Self-test file | `tests/gdd_selftest_failing.rs` |
| `builder_build` | `features/**`, `tests/acceptance/**`, `tests/acceptance.rs` |
| `builder_test` | `src/**`, `crates/*/src/**` |
| Security exceptions | `[advisories] ignore` in `deny.toml`, each with a reason |

### Gate and Learn commands

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

- `types` is empty: clippy type-checks every target. `architecture` is empty: write a
  `cargo tree` rule between the project's crates.
- `mutation`: cargo-mutants on the change's diff, built without debug info, linked with mold
  when it is installed and `RUSTFLAGS` is unset, each test process capped at 2 GB where
  `prlimit` exists. A missed mutant fails. A run where mutants only timed out gets a second
  chance, one at a time with at least 120 s per test; a mutant that hangs again counts as
  caught, and the gate report keeps the command's last line.
- Complexity: clippy's `cognitive_complexity`, as a warning denied by `-D warnings`.

### Tools

| Need | Where | Tool | Alternative |
|---|---|---|---|
| Lint and format | G2 | rustfmt + clippy | |
| Unit tests | G2 | cargo test | cargo-nextest |
| Gherkin runner | G2, G3 | cucumber-rs | |
| Security and licenses | G3 | cargo-deny | cargo-audit (vulnerabilities only) |
| Mutation | G3 | cargo-mutants | |
| Secrets | G3 | gitleaks, on the change's diff | TruffleHog |
| Complexity | G2 | clippy `cognitive_complexity` | |
| Duplication (warning) | Learn | jscpd | |
| Unused dependencies | Learn | cargo-machete | |

### To install

A stable Rust toolchain with rustfmt and clippy, cargo-deny, cargo-mutants, cargo-machete,
gitleaks, and Node.js for `npx jscpd`. Optional: mold (faster mutants) and `prlimit` (the
memory cap, Linux).

CI setup: `dtolnay/rust-toolchain@stable` with rustfmt and clippy, and
`taiki-e/install-action@v2` with cargo-deny and cargo-mutants.

### Skeleton

| File | Holds |
|---|---|
| `Cargo.toml` | The package (edition 2024, `publish = false`), cucumber and futures as development dependencies, the `acceptance` test target |
| `deny.toml` | cargo-deny: advisories, allowed licenses, bans, sources |
| `src/lib.rs` | A first function and its unit tests |
| `tests/acceptance.rs` | The cucumber-rs runner, filtering by `GDD_FEATURES` and `GDD_HIDDEN` |
| `tests/acceptance/world.rs` | The World of the steps |
| `features/.gitkeep` | The scenario folder |

## typescript

TypeScript with npm. Code in `src/`, unit tests in `tests/unit/` (`*.test.ts`), scenario steps
in `features/steps/`.

| Item | Value |
|---|---|
| Scenario runner | cucumber-js |
| Detected by | `package.json`, `tsconfig.json` |
| Supply chain (red zone) | `package-lock.json`, `package.json`, `.npmrc`, `osv-scanner.toml`, `osv-scanner-licenses.toml` |
| `install` | `npm ci` |
| First install (`gdd bootstrap`) | `npm install --no-fund --no-audit` |
| `.gitignore` | `node_modules/`, `reports/`, `coverage/`, `.stryker-tmp/` |
| Self-test file | `tests/unit/gdd-selftest.test.ts` |
| `builder_build` | `features/**` |
| `builder_test` | `src/**` |
| Security exceptions | `[[IgnoredVulns]]` in `osv-scanner.toml`, each with a reason |

### Gate and Learn commands

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

- `security`: development tools included. `dependencies`: the licenses of what ships.
- `mutation`: StrykerJS on the `src/*.ts` files the change touched; the skeleton's
  `stryker.config.json` breaks on any survivor (`thresholds.break = 100`).
- Complexity: ESLint's `complexity` rule (10 in the skeleton), with typescript-eslint and
  eslint-plugin-security.

### Tools

| Need | Where | Tool | Alternative |
|---|---|---|---|
| Lint and format | G2 | ESLint + typescript-eslint | Biome |
| Types | G2 | tsc | |
| Unit tests | G2 | Vitest | Jest |
| Gherkin runner | G2, G3 | cucumber-js | playwright-bdd, for `@ui` scenarios |
| Architecture rule | G2 | dependency-cruiser | |
| Security and licenses | G3 | OSV-Scanner + eslint-plugin-security | npm audit (vulnerabilities only) |
| Mutation | G3 | StrykerJS | |
| Secrets | G3 | gitleaks, on the change's diff | TruffleHog |
| Duplication (warning) | Learn | jscpd | |
| Unused code | Learn | knip | |
| UI and demo | pre | Playwright | |

### To install

Node.js and npm (CI uses Node 22), OSV-Scanner and gitleaks. The rest comes with `npm ci`: the
skeleton's development dependencies are @cucumber/cucumber, @eslint/js, @stryker-mutator/core,
@stryker-mutator/vitest-runner, @types/node, dependency-cruiser, eslint,
eslint-plugin-security, tsx, typescript, typescript-eslint and vitest. jscpd and knip run
through `npx`.

CI setup: `actions/setup-node@v6` with Node 22, `npm ci`, and OSV-Scanner 2.7.0 checked against
its checksums.

### Skeleton

| File | Holds |
|---|---|
| `package.json` | A private ES module package with the development dependencies |
| `tsconfig.json`, `vitest.config.ts` | TypeScript and Vitest settings |
| `eslint.config.js` | typescript-eslint (strict), the security plugin, `complexity` 10 |
| `.dependency-cruiser.cjs` | The architecture rules: no circular imports |
| `stryker.config.json` | StrykerJS with the Vitest runner, every threshold at 100 |
| `cucumber.js` | cucumber-js, strict, steps in `features/steps/**/*.ts` through tsx |
| `features/support/tsx-register.js`, `features/steps/.gitkeep` | Loading TypeScript steps; the step folder |
| `src/greeting.ts`, `tests/unit/greeting.test.ts` | A first module and its unit test |
| `osv-scanner.toml`, `osv-scanner-licenses.toml` | OSV-Scanner settings for `security` and `dependencies` |

## Every stack

Whatever the stacks, the CI workflow `gdd init` writes checks out the repository with its whole
history, runs the setup of each sheet, installs gitleaks 8.30.1 (checked against its checksums)
and gdd itself (`cargo install --locked` from gdd's git repository, at the tag of the version that
wrote the workflow), then runs `bin/gdd ci`.

The `secrets` command and the Rust `mutation` command take the main branch as `main`, or
`origin/main` when there is no local `main`.

## See also

- [commands.toml](configuration.md#commandstoml): what gdd does with each command.
- [Gates](gates.md): when each command runs.
- [Install gdd](../how-to/install.md): the binary, the agents and the tools the gates use.
- [Command reference](cli.md#gdd-stacks): `gdd stacks`, `show` and `compose`.
