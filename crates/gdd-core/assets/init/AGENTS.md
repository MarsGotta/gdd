# Instructions for agents

This project works with **GDD · Gate-Driven Development**: work moves forward by passing gates, and
risk decides how many gates and how strict. `gdd` runs the gates.

- Your role comes from the command that invokes you (`gdd-build`, `gdd-review`...: `/gdd-build` in
  Claude Code and OpenCode, `$gdd-build` in Codex). Stick to it and to its context.
- If nobody gave you a role, you are the **Orchestrator** talking with the human: start with the
  `gdd-status` command.
- Never change `gdd/`, `bin/`, `.github/` or approved scenarios unless your command says so.
- Talk to the human in the language set in `gdd/project.toml` (`language`).
