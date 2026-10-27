---
description: "Python in a GDD project: steps with pytest-bdd, unit tests, what the gates catch"
---

# GDD · Python

Read this before writing tests, scenario steps or code in Python in a GDD project.

## Scenario steps (pytest-bdd)

- The steps live in `tests/acceptance/`; the `.feature` files in `features/`. When the project keeps them elsewhere, `builder_build`
  in `gdd/rules.toml` says where. The test module
  binds the features with `scenarios(...)`, on `GDD_FEATURES` (one path per line; all when empty)
  and on `features/hidden/` only when `GDD_HIDDEN=1`. A sentence with no step fails
  (`StepDefinitionNotFoundError`). Never skip, comment out or loosen a scenario to make it pass.
- One step per sentence with `@given`, `@when`, `@then` and `parsers.parse(...)` for values; share
  state through fixtures (`target_fixture`), not globals.
- Steps talk to the package in `src/`, never to a shell. Each scenario builds its own state
  (`tmp_path`). Never read the hidden examples.

## Unit tests

- In `tests/unit/`, `test_<module>.py`, plain `assert` with pytest. Names say the behavior:
  `test_a_change_without_paths_is_green`.

## What the gates usually catch in Python

- **Mutation testing** (mutmut, G3) changes conditions, operators and returns in the modules the
  change touched; any survivor fails the gate. Every branch, including each `except`, needs a test
  that would fail without it; catching a broad `Exception` makes that hard: catch what you expect.
- **mypy**: type every public function; no `Any` leaking from input; handle `None`.
- **Ruff** (with `C901` for complexity) and **Bandit**: small functions, no `shell=True`, no
  `eval`, no paths or commands built from input without checking them.
- **import-linter**: respect the contracts in `.importlinter`.

## Never by hand

Do not run `pytest`, `ruff`, `mypy` or `mutmut` yourself to decide whether you are done: run `bin/gdd g2 --role <your role>`.
It runs the same tools with the project's flags, Path Guard and the scope, and it is what counts.
Read its report (`.gdd/changes/<id>/g2.json`) when it fails.
