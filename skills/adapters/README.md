# Adapters

They're optional. Each one maps a role that runs as its own agent to its skill, its tools and its level, for an agent that supports it. They hold no code, and the method doesn't depend on them: without them, the orchestrator names the skill when it launches each agent.

## Claude Code

Copy `claude-code/agents/` to `.claude/agents/` in your project. Each subagent preloads its skill, so it starts with its role and nothing else. They say `model: inherit` because the level depends on the change's color: the orchestrator launches each one with the model the blueprint gives that level.

## Codex

There's nothing to copy. Each role's skill carries `agents/openai.yaml` with `allow_implicit_invocation: false`, so it loads only when it's named, as in `$gdd-build`.
