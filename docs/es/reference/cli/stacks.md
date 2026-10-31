# gdd stacks

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/stacks.md, nunca esta página.
-->

The stack sheets gdd comes with (python, rust, typescript): one line per sheet.

## Uso

```text
Usage: gdd stacks [COMMAND]
```

## Opciones

Sin opciones, salvo `-h`, `--help`.

## Subcomandos

| Subcomando | Descripción |
|---|---|
| [`gdd stacks show`](stacks-show.md) | One sheet: the tool for each need, the commands and the protected paths. |
| [`gdd stacks compose`](stacks-compose.md) | What the named sheets give together, as TOML: the shape of gdd/commands.toml plus a [protected] table. Writes nothing. |

## Quién lo ejecuta

You, to see which stacks [`gdd init`](init.md) accepts in `--stack`.

## Ejemplos

```bash
gdd stacks
```

You should see one line per sheet (name, technology, scenario runner):

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
python · Python · pytest-bdd
rust · Rust · cucumber-rs
typescript · TypeScript · cucumber-js
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Stacks](../stacks.md): what each sheet holds.
