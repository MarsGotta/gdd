# Skills

Skills for your coding agent, so you can try GDD in your own project before T-minus is ready. This is version 0.2: the scenarios in [`evals/`](../evals/) try them with a real model, and they don't cover every skill yet. What was decided about them, and why, is in [DESIGN.md](DESIGN.md), and what they leave in your repository is in [CONTRACT.md](CONTRACT.md).

| Skill | Who loads it | Moment |
|---|---|---|
| `gdd` | The orchestrator, on its own, when you ask for any change | All four |
| `gdd-prepare` | The orchestrator, with you | Prepare |
| `gdd-brief` | The orchestrator | Brief |
| `gdd-checks` | Another agent, never the one that builds | Brief, and after Prepare and Harden |
| `gdd-build` | The agent that goes through the gate | Execute |
| `gdd-review` | The reviewer, with a clean context | Execute |
| `gdd-harden` | The orchestrator, with you | Harden |

## Installing them

Copy the seven folders whose names start with `gdd` into your project: `.agents/skills/` for Codex, `.claude/skills/` for Claude Code. For Claude Code you can also copy the subagents in [adapters/](adapters/). Then ask your agent to set GDD up, or ask it for a change, and `gdd` takes it from there.

On Claude Code 2.1.275 or later you can install them as a plugin instead, in one step:

```
/plugin install gdd --marketplace MarsGotta/gdd
```

It brings the seven skills and the three subagents, under the names `gdd:gdd`, `gdd:gdd-build` and so on, and it updates when this repository does. It carries no code.

They install nothing and run nothing of their own: they use what your project already has.

## Keeping them in line

`python3 scripts/check_skills.py`, from the root of this repository, checks that every skill's templates match the contract and that each skill keeps to its design. `python3 scripts/check_commands.py` runs the git commands the skills give in a throwaway repository. And [`evals/`](../evals/) holds scenarios that check, with a real model, how an agent behaves with the skills. All of it is for this repository: your project doesn't need it.
