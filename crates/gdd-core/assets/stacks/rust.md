---
description: "Rust in a GDD project: steps with cucumber-rs, unit tests, what the gates catch"
---

# GDD · Rust

Read this before writing tests, scenario steps or code in Rust in a GDD project.

## Scenario steps (cucumber-rs)

- The steps live in `tests/acceptance/` (the `acceptance` test target, `harness = false`); the
  `.feature` files in `features/`. When the project keeps them elsewhere, `builder_build`
  in `gdd/rules.toml` says where. The runner is strict: a sentence with no step fails the run.
  Never skip, comment out or loosen a scenario to make it pass.
- One step per sentence, matched by `#[given]`, `#[when]`, `#[then]` with a cucumber expression
  (`{string}`, `{int}`) or a regex. An `And` takes the kind of the step before it.
- Steps talk to the library (the crate's public API), never to the terminal or a shell. Each
  scenario builds its own temporary project or state; nothing is shared between scenarios, and no
  step changes the process environment (pass it explicitly).
- Run only `GDD_FEATURES` (one path per line; all when empty) and `features/hidden/` only with
  `GDD_HIDDEN=1`. Never read the hidden examples.

## Unit tests

- Next to the code, in `#[cfg(test)] mod tests` at the end of each module. Names say the
  behavior: `a_change_without_paths_is_green`, not `test_1`.
- Integration tests that need the binary go in `tests/`, not in the acceptance target.

## What the gates usually catch in Rust

- **Mutation testing** (cargo-mutants, G3) replaces match guards, conditions and returns. Every
  error branch needs its own test: a guard such as `e.kind() == ErrorKind::NotFound` needs a test
  with a different error kind, or the mutant that turns it into `true` survives.
- **clippy** with `-D warnings`: no `unwrap()` or `expect()` on input; errors carry what failed and
  where (`cannot read <path>: <reason>`).
- **cargo-deny**: a new dependency must be in the plan; a new license must be allowed.
- Keep `cognitive_complexity` low: small functions with a name each.

## Never by hand

Do not run `cargo test`, `cargo clippy` or `cargo mutants` yourself to decide whether you are done: run `bin/gdd g2 --role <your role>`.
It runs the same tools with the project's flags, Path Guard and the scope, and it is what counts.
Read its report (`.gdd/changes/<id>/g2.json`) when it fails.
