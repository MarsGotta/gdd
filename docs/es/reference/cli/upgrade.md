# gdd upgrade

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/upgrade.md, nunca esta página.
-->

Proposes what this gdd writes of the files gdd init wrote once (gdd/generated.toml lists them)
on the branch gdd-upgrade, merged three ways with what the project made of them: first what
this gdd writes, then the merge, with the lines both changed marked as conflicts. Runs on the
main branch with everything committed; nothing on it changes and nothing is pushed. Builders
and Reviewers cannot run it.

## Uso

```text
Usage: gdd upgrade
```

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Quién lo ejecuta

You, after you install a new gdd, on the main branch with everything committed.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#qué-puede-ejecutar-un-agente).

## Ejemplos

It proposes the files the new gdd writes, merged with your edits, on a branch `gdd-upgrade`.
Nothing is pushed: you review and merge the branch yourself.

```bash
gdd upgrade
```

When the project already has what this gdd writes, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
Already up to date with gdd 0.1.0: nothing to propose.
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Upgrade gdd](../../how-to/upgrade.md): the whole path, conflicts included.
