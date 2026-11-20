# Adapters

They're optional. Each one maps a role that runs as its own agent to its skill, its tools and its level, for an agent that supports it. They hold no code, and the method doesn't depend on them: without them, the prompt that `gdd launch` gives names the skill of each agent.

## Claude Code

Copy `claude-code/agents/` to `.claude/agents/` in your project. Each subagent preloads its skill, so it starts with its role and nothing else. They say `model: inherit` because the level depends on the change's color: the orchestrator launches each one with the model the blueprint gives that level.

Each one has the tools of its work and no more. The three that write and run have six. Two only read, so they have `Read`, `Grep` and `Glob`: `gdd-read`, which reads a brief before anyone builds it, and `gdd-scout`, which searches the project for the orchestrator while it writes a brief and answers with a list of paths.

The two that judge, `gdd-review` and `gdd-read`, say `omitClaudeMd: true`: they start without the instruction files of the project and of the person, so nothing reaches them but what they are given. The constitution still does, because the layer writes it in their prompt as it stood at `<start>`. It needs Claude Code 2.1.271 or later.

They don't keep the builder away from what judges it. Claude Code has no permission rule on paths for one subagent alone: those rules live in the project's settings and bind every agent in the session, the orchestrator included. So the builder's limits stay where the method puts them: the `seal` check, the platform's protections and the agent's sandbox.

## Codex

There's nothing to copy. Each role's skill carries `agents/openai.yaml` with `allow_implicit_invocation: false`, so it loads only when it's named, as in `$gdd-build`, or `$gdd:gdd-build` when the skills came as a plugin. The orchestrator reaches the four skills it runs itself (`gdd-prepare`, `gdd-brief`, `gdd-close` and `gdd-harden`) by opening their files, which sit next to its own. Both were tried with Codex CLI 0.166.0.

## Models

The blueprint's `levels` name, for each agent the project is worked from, the model of each level and how a role gets it. Where to look when preparing (checked on 2026-11-11 with Claude Code 2.1.319, Codex CLI 0.166.0 and Antigravity 1.5.5):

| Agent | Lists its models | A role as a new session, on a model |
|---|---|---|
| Claude Code | No command: `--model` takes an alias or a full name | `claude -p --model <model> --output-format json` |
| Codex | `codex debug models` | `codex exec --json -m <model>` |
| Antigravity | `agy models` | `agy -p --model <model>` |

A role that only reads can be launched so that it can't write or run anything, and that goes in the blueprint's `read_only`: `codex exec --json --sandbox read-only -m <model>` with Codex, and `--tools` with only the reading tools and the one that loads a skill with Claude Code. Both options exist in Codex CLI 0.170.0 and Claude Code 2.1.327. A subagent's `omitClaudeMd` doesn't apply to a session launched this way.

Written that way in the blueprint's `launch`, `gdd launch <id> <role> --run` puts the model and the prompt in, runs it with its standard input closed, checks within a minute that it wrote something, stops it past the minutes the change has left, and writes the role's step with the tokens the output reports: the flags for JSON output are what let it read them on Claude Code and Codex. Without them it still reads the role's last message, which is what the command writes to its standard output, and no tokens. Launched by hand, close its standard input yourself, `< /dev/null`, and check the same, so a role that runs in the background can't sit waiting for a keyboard nobody is at. In one run, a reviewer launched in the background from another agent's command line waited for input until the person asked what was happening.
