# gdd release

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/release.md, nunca esta página.
-->

A release step of gdd/commands.toml: pre (tests and demo), promote (to production, after G4) or
observe (rolls the change back when production gets worse). Prints it as JSON. In [versions]
mode = "auto" of gdd/rules.toml (the default), promote gives a change with a changelog line the
next v<x.y.z> version, in GDD_VERSION, and releases it once it passed. Without a step: the next
version of what main merged since its newest tag, and its lines; --yes releases it, in any mode
(CHANGELOG.md, when main tracks one, in a commit of its own, and the tag), and --version
chooses it. Builders and Reviewers cannot release.

## Uso

```text
Usage: gdd release [OPTIONS] [STEP] [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[STEP]` | The step; without one, the next version of what is merged. Valores posibles: `pre`, `promote`, `observe`. |
| `[CHANGE]` | Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE. |

## Opciones

| Opción | Descripción |
|---|---|
| `--yes` | Release the next version, without asking. |
| `--version <X.Y.Z>` | The version to release instead of the next one, after the newest: x.y.z. |

## Quién lo ejecuta

[`gdd launch`](launch.md) runs the steps; you run `gdd release` without a step.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

`pre` (tests and demo) runs after the merge, `promote` after G4 and `observe` after the
promotion. Without a step, it shows the next version of what main merged; `--yes` releases it.

```bash
gdd release
```

With nothing to release, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Nothing to release: no change on main carries a changelog line
```

The [tutorial](../../getting-started/tutorial.md) shows the steps of a real release.

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Release versions and keep a changelog](../../how-to/releases-and-changelog.md): tags,
  `--yes` and `--version`.
