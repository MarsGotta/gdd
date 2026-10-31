# gdd changelog

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/changelog.md, nunca esta página.
-->

Shows what CHANGELOG.md lacks of the lines the commits of the main branch carry (what no
v<semver> tag released yet goes under Unreleased), and with --write adds only that, each line
with a comment naming its commit. Every word of the file stays; nothing is committed. In a
terminal it asks before it writes.

## Uso

```text
Usage: gdd changelog [OPTIONS]
```

## Opciones

| Opción | Descripción |
|---|---|
| `--write` | Add what is missing to CHANGELOG.md, without asking. |
| `--check` | Fail when something is missing, for CI; writes nothing. |

## Quién lo ejecuta

You, before a release, and CI, with `gdd changelog --check`, which fails when a line is
missing.

## Ejemplos

It only adds lines: it never removes or rewrites a word of `CHANGELOG.md`. In a terminal it
asks before it writes.

```bash
gdd changelog
```

When nothing is missing, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
CHANGELOG.md covers every change on main.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Release versions and keep a changelog](../../how-to/releases-and-changelog.md): the whole
  path.
