---
summary: Shows the changes of the project by phase, on a read-only screen.
runs: you
---

## Who runs it

You, in a terminal.

## Examples

The Board shows every change by phase (Discover, Build, Release, Done) and changes nothing.
`gdd` with no command opens it when a person runs it in a terminal. Without a terminal, or for
an agent, `gdd` alone prints the help.

```bash
gdd board
```

In a project with no change yet, you should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
┌ gdd · Board · shop ──────────────────────────────────────────────────────────────────────────────┐
│No changes yet · ask your agent: /gdd-change <what you want> ($gdd-change in Codex)               │
│↑↓ move · Enter details · d done · t time · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Without a terminal, it names `gdd status` and exits with 2:

```text
error: gdd board needs a terminal; gdd status lists the same changes
```

## See also

- [Screens](../screens.md#the-board): the Board and its keys.
- [`gdd status`](status.md): the same changes as text.
