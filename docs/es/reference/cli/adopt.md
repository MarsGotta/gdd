# gdd adopt

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/adopt.md, nunca esta página.
-->

Adopts a project that already has code: runs once each G2 and G3 command that judges the whole
project (not acceptance*, mutation* or secrets*), keeps what each failing one said in
.gdd/adoption/, records them as known debt in gdd/debt.toml, which it commits alone, and
proposes a Refactor per command, riskiest first. Refused once gdd/debt.toml exists.

## Uso

```text
Usage: gdd adopt
```

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

Your agent runs it once, in the [`gdd-setup`](../skills/setup.md) skill, in a repository that
already has code. You can also run it after [`gdd init`](init.md) and the first install. An
agent that gdd started cannot run it: see [What an agent may
run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It records what already fails as known debt. From then on, a change that makes that debt
worse fails its gate.

```bash
gdd adopt
```

In a project whose code had an unused import, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, a Python project that already had code, in a real gdd-setup
     session. -->

```text
Passed: architecture, types, unit, dependencies, lockfile, security
Known debt: lint (1)
Proposed Refactors, riskiest first:
  1. debt-lint · Pay the known debt of lint · .gdd/adoption/lint.txt
Ask your agent for each one: /gdd-change pay the debt of <name> ($gdd-change in Codex).
```

A second run refuses:

```text
error: this project was already adopted on 2026-10-31 (gdd/debt.toml)
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Bring gdd to an existing project](../../how-to/adopt-existing-project.md): the whole path.
