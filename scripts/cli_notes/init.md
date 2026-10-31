---
summary: Sets up GDD in a folder: the harness, the Brief, the agents' skills, a stack skeleton.
runs: you
---

## Who runs it

You, once per project, in the folder that becomes the project root.
An agent that gdd started cannot run it: see
[What an agent may run](../roles.md#what-an-agent-may-run).

## Examples

In a terminal, `gdd init` with no flags opens the Setup form, one step each for the name, the
description, the platform, the stacks, the agents, the language and the sensitive areas. On
`Enter` it writes and commits the harness, runs the first install of each stack as
[`gdd bootstrap`](bootstrap.md) does, and commits the lock files. The next step is the
[`gdd-setup`](../skills/setup.md) skill in your agent (`/gdd-setup` in Claude Code and
OpenCode, `$gdd-setup` in Codex). With `--name`, `--platform` and `--stack` it asks nothing and
installs nothing; then [`gdd bootstrap`](bootstrap.md) or the `gdd-setup` skill runs the
install.

Open the Setup form in a terminal:

```bash
gdd init
```

After the summary, `Enter` lists every file it wrote, the install, and the next step:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
gdd/project.toml
gdd/commands.toml
...
.agents/skills/gdd-stack-python/SKILL.md
python: uv lock && uv sync --locked
...
Committed: uv.lock
Next: start your coding agent here and run the gdd-setup command (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

Set up a Python command-line project without the form, for Claude Code and Codex:

```bash
gdd init --name shop --platform cli --stack python --agents claude codex
```

You should see every file it wrote, then what to run next:

```text
gdd/project.toml
gdd/commands.toml
gdd/rules.toml
gdd/zones.toml
gdd/models.toml
gdd/generated.toml
docs/brief.md
...
Next: run gdd bootstrap (first install of each stack and its lock file), then start your agent and run the gdd-setup command (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

Inside a project, it refuses:

```text
error: already a GDD project: /home/me/shop/gdd/project.toml
```

## See also

- [Screens](../screens.md#the-setup-form): the Setup form, step by step.
- [Install gdd](../../how-to/install.md): what you need before `gdd init`.
- [Bring gdd to an existing project](../../how-to/adopt-existing-project.md): `gdd init` in a
  folder that already has code.
