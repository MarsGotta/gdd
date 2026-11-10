# Skills

Skills for your coding agent, so you can try GDD in your own project before T-minus is ready. This is their first version. What was decided about them, and why, is in [DESIGN.md](DESIGN.md), and what they leave in your repository is in [CONTRACT.md](CONTRACT.md).

| Skill | Who loads it | Moment |
|---|---|---|
| `gdd` | The orchestrator, on its own, when you ask for any change | All four |
| `gdd-prepare` | The orchestrator, with you | Prepare |
| `gdd-brief` | The orchestrator | Brief |
| `gdd-checks` | Another agent, never the one that builds | Brief and Execute |
| `gdd-build` | The agent that goes through the gate | Execute |
| `gdd-review` | The reviewer, with a clean context | Execute |
| `gdd-harden` | The orchestrator, with you | Harden |

## Installing them

Copy the seven folders whose names start with `gdd` into your project: `.agents/skills/` for Codex, `.claude/skills/` for Claude Code. For Claude Code you can also copy the subagents in [adapters/](adapters/). Then ask your agent to set GDD up, or ask it for a change, and `gdd` takes it from there.

They install nothing and run nothing of their own: they use what your project already has.

## Keeping them in line

`python3 scripts/check_skills.py`, from the root of this repository, checks that every skill's templates match the contract and that each skill keeps to its design. It's for this repository: your project doesn't need it.
