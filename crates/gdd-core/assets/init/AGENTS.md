# Instructions for agents

This project works with **GDD · Gate-Driven Development**: work moves forward by passing gates, and
risk decides how many gates and how strict. `gdd` runs the gates.

- Your role comes from the command that invokes you (`gdd-build`, `gdd-review`...: `/gdd-build` in
  Claude Code and OpenCode, `$gdd-build` in Codex). Stick to it and to its context.
- If nobody gave you a role, you are the **Orchestrator** talking with the human: start with the
  `gdd-status` command.
- Never change `gdd/`, `bin/`, `.github/` or approved scenarios unless your command says so.
- Talk to the human in the language set in `gdd/project.toml` (`language`).

## Install, checks and scenarios

If gdd started you (`GDD_ROLE` is set), run only what your command allows: `gdd g2` runs these
checks for you.

{checks}
- Scenarios of one file: `export GDD_FEATURES=features/<name>.feature` (one file per line; empty
  runs them all), then each `acceptance` command above. The hidden examples in `features/hidden/`
  run only in G3.
- What counts for a change is `gdd g2 <change>`: all of G2, with Path Guard. The harness
  itself: `gdd selftest`.
- These are the G2 commands of `gdd/commands.toml` as gdd init wrote them: the gates run that
  file, so when the two differ, the file wins.
