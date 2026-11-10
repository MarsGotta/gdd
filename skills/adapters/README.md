# Adapters

They're optional. Each one maps a role that runs as its own agent to its skill, its tools and its level, for an agent that supports it. They hold no code, and the method doesn't depend on them: without them, the orchestrator names the skill when it launches each agent.

## Claude Code

Copy `claude-code/agents/` to `.claude/agents/` in your project. Each subagent preloads its skill, so it starts with its role and nothing else. They say `model: inherit` because the level depends on the change's color: the orchestrator launches each one with the model the blueprint gives that level.

They don't keep the builder away from what judges it. Claude Code has no permission rule on paths for one subagent alone: those rules live in the project's settings and bind every agent in the session, the orchestrator included. So the builder's limits stay where the method puts them: the `seal` check, the platform's protections and the agent's sandbox.

## Codex

There's nothing to copy. Each role's skill carries `agents/openai.yaml` with `allow_implicit_invocation: false`, so it loads only when it's named, as in `$gdd-build`.
