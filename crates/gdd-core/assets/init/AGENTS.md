# Instructions for agents

This project works with **GDD · Gate-Driven Development**: work moves forward by passing gates, and
risk decides how many gates and how strict. `tminus` runs the gates.

- Your role comes from the command that invokes you (`tminus-build`, `tminus-review`...: `/tminus-build` in
  Claude Code and OpenCode, `$tminus-build` in Codex). Stick to it and to its context.
- If nobody gave you a role, you are the **Orchestrator** talking with the human: start with the
  `tminus-status` command.
- Never change `tminus/`, `bin/`, `.github/` or approved scenarios unless your command says so.
- Talk to the human in the language set in `tminus/project.toml` (`language`).

## Install, checks and scenarios

If tminus started you (`TMINUS_ROLE` is set), run only what your command allows: `tminus g2` runs these
checks for you.

{checks}
- Scenarios of one file: `export TMINUS_FEATURES=features/<name>.feature` (one file per line; empty
  runs them all), then each `acceptance` command above. The hidden examples in `features/hidden/`
  run only in G3.
- What counts for a change is `tminus g2 <change>`: all of G2, with Path Guard. The harness
  itself: `tminus selftest`.
- These are the G2 commands of `tminus/commands.toml` as tminus init wrote them: the gates run that
  file, so when the two differ, the file wins.
