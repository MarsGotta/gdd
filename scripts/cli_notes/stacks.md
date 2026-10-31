---
summary: Lists the stack sheets gdd comes with.
runs: you
---

## Who runs it

You, to see which stacks [`gdd init`](init.md) accepts in `--stack`.

## Examples

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

## See also

- [Stacks](../stacks.md): what each sheet holds.
