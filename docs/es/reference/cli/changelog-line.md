# gdd changelog-line

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/changelog-line.md, nunca esta página.
-->

Sets the line of a change in the changelog, which its merge commit carries as trailers: a
section of Keep a Changelog and its words, or --none to leave the change out. Without one, the
title goes under the section of its type (Feature: Added, Hotfix: Fixed, Migration: Changed),
and the other types are left out. Builders and Reviewers cannot set it.

## Uso

```text
Usage: gdd changelog-line [OPTIONS] <CHANGE> [SECTION] [LINE]...
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `<CHANGE>` | Id of the change: its record is .gdd/changes/<id>/change.json. |
| `[SECTION]` | The section: Added, Changed, Deprecated, Removed, Fixed or Security. |
| `[LINE]...` | The line, for the people who use the project; its words are joined with single spaces. |

## Opciones

| Opción | Descripción |
|---|---|
| `--breaking` | The change breaks what worked before. |
| `--none` | Leave the change out of the changelog. |

## Quién lo ejecuta

You, before the change merges.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It sets the line that replaces the change's title in the changelog; `--none` leaves the change
out.

```bash
gdd changelog-line cart-total Added "The cart shows its total"
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Changelog line of cart-total: Added · The cart shows its total
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Release versions and keep a changelog](../../how-to/releases-and-changelog.md): sections,
  `--breaking` and `--none`.
