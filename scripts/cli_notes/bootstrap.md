---
summary: Runs the first install of each stack and commits its lock file.
runs: you, your agents
---

## Who runs it

You, after `gdd init` with flags, or after you fix an install that failed in the Setup form of
[`gdd init`](init.md). The [`gdd-setup`](../skills/setup.md) skill also runs it when a stack's
lock file is missing. An agent that gdd started cannot run it: see [What an agent may
run](../roles.md#what-an-agent-may-run).

## Examples

It runs the `bootstrap` command of each stack sheet, so each stack's tool must be on your
`PATH`: uv for Python, cargo for Rust, npm for TypeScript. The Setup form of `gdd init` runs the
same install.

```bash
gdd bootstrap
```

After `gdd init` with flags, you should see each stack's install, then the files it committed:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
python: uv lock && uv sync --locked
Using CPython 3.13.15
Resolved 40 packages in 250ms
...
Committed: uv.lock
Next: in your coding agent, the gdd-setup command finishes the Setup (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

A second run installs and commits nothing new:

```text
python: uv lock && uv sync --locked
Resolved 40 packages in 1ms
Resolved 40 packages in 0.93ms
Checked 39 packages in 0.57ms
Nothing new to commit.
Next: in your coding agent, the gdd-setup command finishes the Setup (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

## See also

- [Install gdd](../../how-to/install.md): the tools each stack needs.
- [Troubleshooting](../../how-to/troubleshooting.md#the-first-install-failed): when the first
  install fails.
