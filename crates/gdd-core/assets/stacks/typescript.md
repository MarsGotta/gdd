---
description: "TypeScript in a GDD project: steps with cucumber-js, unit tests, what the gates catch"
---

# GDD · TypeScript

Read this before writing tests, scenario steps or code in TypeScript in a GDD project.

## Scenario steps (cucumber-js)

- The steps live in `features/steps/` (`*.ts`, loaded through the `import` setting of the cucumber
  configuration); the `.feature` files in `features/`. When the project keeps them elsewhere, `builder_build`
  in `gdd/rules.toml` says where. Runs are `--strict`: a sentence with no step
  or a pending step fails. Never skip, comment out or loosen a scenario to make it pass.
- One step per sentence with `Given`, `When`, `Then` from `@cucumber/cucumber` and a cucumber
  expression (`{string}`, `{int}`). Keep state in the World (`this`), never in module variables.
- Steps talk to the application's modules, not to a browser, unless the scenario is tagged `@ui`
  (Playwright). Each scenario builds its own state.
- G2 runs only `GDD_FEATURES` and skips `@hidden`; G3 runs everything. Never read the hidden
  examples.

## Unit tests

- In `tests/unit/`, one `<module>.test.ts` per module, with Vitest (`describe`, `test`, `expect`).
  Names say the behavior.
- No snapshot tests for logic: assert the value.

## What the gates usually catch in TypeScript

- **Mutation testing** (StrykerJS, G3) changes conditions, operators and returns in the files the
  change touched; any survivor fails the gate. Every branch, including each error path and each
  `catch`, needs a test that would fail without it.
- **tsc** in strict mode: no `any`, no non-null assertions (`!`) on input; handle `undefined`.
- **ESLint** with the security plugin and the complexity rule: small functions, no `eval`, no
  building commands or paths from input without checking them.
- **dependency-cruiser**: respect the layer rules of `.dependency-cruiser.cjs`.

## Never by hand

Do not run `vitest`, `eslint`, `tsc` or `stryker` yourself to decide whether you are done: run `bin/gdd g2 --role <your role>`.
It runs the same tools with the project's flags, Path Guard and the scope, and it is what counts.
Read its report (`.gdd/changes/<id>/g2.json`) when it fails.
