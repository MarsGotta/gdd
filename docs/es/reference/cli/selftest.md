# gdd selftest

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/selftest.md, nunca esta página.
-->

Harness self-test: a failing test turns a green G2 red, the harness is protected and CI runs
the gates, rollback works in pre, and zero scenarios fail G3. Saved in .gdd/selftest.json and
logged; exits with 1 unless all four pass.

## Uso

```text
Usage: gdd selftest
```

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

Your agent, as the Orchestrator, in the [`gdd-setup`](../skills/setup.md) skill before it opens
G0, and you, after you change `gdd/commands.toml`. An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It breaks the harness on purpose and checks that the gates notice.

```bash
gdd selftest
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
OK a failing test turns G2 red
OK editing a protected file is blocked (Path Guard and CI)
OK rollback works in pre (simulated)
OK zero scenarios counts as a failure
harness 4/4
```

A Builder that tries it gets:

```text
error: Builders and Reviewers cannot run gdd selftest: an agent gdd started may only run gdd g2, gdd guard, gdd note, gdd status, gdd classify, gdd inbox, gdd report and gdd scope.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Gates](../gates.md#g0-setup-gate): G0 and its four checks.
