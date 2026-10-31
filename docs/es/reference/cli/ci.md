# gdd ci

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/ci.md, nunca esta página.
-->

What CI runs: on the branch change/<id> (GITHUB_HEAD_REF, else the current one), no harness
path changed against the main branch, and G2 and G3 green. Reads and writes nothing under
.gdd/; exits with 1 when CI fails.

## Uso

```text
Usage: gdd ci
```

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

CI: the workflow that [`gdd init`](init.md) writes, `.github/workflows/gdd.yml`, runs it on
every pull request and every push to `main` and `change/**`. You can run it to see what CI will
report.

## Ejemplos

It writes nothing under `.gdd/`.

```bash
gdd ci
```

On the main branch there is no change to check:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Branch main: nothing to check per change.
```

On the branch of a change that edited a harness file and whose scenarios have no steps yet, it
lists each problem and exits with 1:

```text
gdd/rules.toml: harness path changed inside a change
G2 red: acceptance
G3 red: acceptance_full
CI failed
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Run the gates in CI](../../how-to/ci.md): the workflow and what to add to it.
