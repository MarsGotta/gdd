# Install gdd

Follow this guide once on each machine where you or your agents will use GDD. You install the
`gdd` binary, at least one coding agent and the tools your stack's gates run, then set up a
project with `gdd init`.

## Before you start

- Linux or macOS. gdd runs its gates through `sh` and the agents in git worktrees.
- [git](https://git-scm.com), with your name and email set (`git config --global user.name`
  and `user.email`): gdd commits what it writes.
- [Rust](https://rustup.rs) 1.98 or newer, to build gdd. If your toolchain is older,
  `rustup update` brings it up to date.
- The tools of your stack. gdd's [gates](../glossary.md#gate) run real programs (your linter,
  your tests, a security scanner), so a gate fails when its program is missing. You install
  these yourself; the rest comes with the project's dependencies (step 3 has the detail):

| Stack | Install yourself |
|---|---|
| Python | uv, OSV-Scanner, gitleaks; Node.js for one Learn check |
| Rust | cargo-deny, cargo-mutants, gitleaks; cargo-machete and Node.js for Learn |
| TypeScript | Node.js and npm, OSV-Scanner, gitleaks |

The [stacks reference](../reference/stacks.md) lists every command each stack's gates run.

gdd 0.1.0 installs with cargo only. Prebuilt binaries, crates.io and Homebrew are on the
[roadmap](../../ROADMAP.md#5-open-source) and not available yet.

## 1. Install the binary

Install the tagged version from GitHub. The CI (continuous integration) workflow that
`gdd init` writes runs this same command:

```bash
cargo install --locked --git https://github.com/MarsGotta/gdd --tag v0.1.0 gdd
```

cargo builds gdd and puts it in `~/.cargo/bin`, which rustup adds to your `PATH`. You, your
agents and CI all run this one `gdd`.

> [!NOTE]
> The workflow `gdd init` writes installs gdd from the tag `v<version>`, so CI works once
> that tag exists on GitHub. If cargo says `failed to find tag v0.1.0`, the tag is not
> published yet: install from the main branch with the same command without `--tag v0.1.0`.

### Or build from source

If you prefer a local copy, clone the repository and install from the checkout:

```bash
git clone https://github.com/MarsGotta/gdd
cd gdd
cargo install --locked --path crates/gdd
```

The repository pins its toolchain in `rust-toolchain.toml`, so rustup downloads Rust 1.98.1
the first time you build there. If you want to work on gdd itself, follow
[CONTRIBUTING.md](../../CONTRIBUTING.md) instead: it builds the binary the project's own
gates use.

### Check the binary

```bash
gdd --version
```

You should see:

```text
gdd 0.1.0
```

## 2. Install a coding agent

gdd starts the command-line program of each coding agent, headless (with no window), in the
role it gives it. You talk to the same agents through the GDD skills, such as `/gdd-change`.
gdd knows three agents, with one data sheet each:

| Agent | Program on the `PATH` | Skills written to | Called as |
|---|---|---|---|
| [Claude Code](https://code.claude.com/docs) | `claude` | `.claude/skills/` | `/gdd-status` |
| [Codex CLI](https://developers.openai.com/codex) | `codex` | `.agents/skills/` | `$gdd-status` |
| [OpenCode](https://opencode.ai/docs) | `opencode` | `.opencode/commands/` | `/gdd-status` |

Install the ones you want with their own instructions, and sign in to each once,
interactively. gdd uses those logins, so a subscription works. gdd never passes your API
(application programming interface) keys to an agent:
[Environment variables](../reference/environment.md#which-variables-hold-a-secret) has the
rule.

Install agents of **two model families** if you can: for example Claude Code to build and Codex
to review. The Reviewer must be of another family than the Builder, so that it does not miss
the same mistakes; [Keeping agents honest](../explanation/safety.md#layer-2-judges-the-builder-cannot-reach)
explains why. If you have one family only, the Setup form asks for an explicit yes, and
`gdd init` with flags refuses and says what to do:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

To change agents or models later, see [Choose agents and models](agents-and-models.md).

## 3. Install the tools of your stack

Each [stack sheet](../glossary.md#stack-sheet) names the commands its gates run. Some tools are
dev dependencies of the project: the Setup form of `gdd init` installs them when it runs the
first install of each stack. You install the others yourself, on your `PATH`. In the tables,
G3 is the Merge Gate, the checks a change passes before it merges, and Learn is the
[phase](../glossary.md#learn) that turns repeated problems into rules.

### Python

| Need | Tool | How it gets there |
|---|---|---|
| Environment, every command | [uv](https://docs.astral.sh/uv/) | install it yourself |
| Lint, types, unit tests, scenarios, architecture, security, mutation, unused code | Ruff, mypy, pytest, pytest-bdd, import-linter, Bandit, mutmut, vulture | dev dependencies in `pyproject.toml`; the Setup form installs them |
| G3 security and licenses | [OSV-Scanner](https://google.github.io/osv-scanner/) | install it yourself |
| G3 secrets | [gitleaks](https://github.com/gitleaks/gitleaks) | install it yourself |
| Learn: duplication (warning only) | jscpd, through `npx` | needs [Node.js](https://nodejs.org) |

### Rust

| Need | Tool | How it gets there |
|---|---|---|
| Lint, format, unit tests, scenarios | rustfmt, clippy, cargo test, cucumber-rs | rustup components; cucumber is a dev dependency |
| G3 security, sources and licenses | [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | `cargo install --locked cargo-deny` |
| G3 mutation testing | [cargo-mutants](https://mutants.rs) | `cargo install --locked cargo-mutants` |
| G3 secrets | gitleaks | install it yourself |
| Learn: unused dependencies, duplication | cargo-machete, jscpd through `npx` | `cargo install --locked cargo-machete`; Node.js |

Two tools are optional on Rust. If [mold](https://github.com/rui314/mold) is installed,
mutation testing links with it. If `prlimit` exists (Linux), it caps each test process.

### TypeScript

| Need | Tool | How it gets there |
|---|---|---|
| Every command | Node.js and npm | install them yourself |
| Lint, types, unit tests, scenarios, architecture, mutation | ESLint, tsc, Vitest, cucumber-js, dependency-cruiser, StrykerJS | dev dependencies in `package.json`; the Setup form installs them |
| G3 security and licenses | OSV-Scanner | install it yourself |
| G3 secrets | gitleaks | install it yourself |
| Learn: unused code, duplication | knip, jscpd, through `npx` | fetched by `npx` |

The [stacks reference](../reference/stacks.md) lists every command each sheet runs.

## 4. Set up a project

1. Open a terminal in a new folder, or in the root of a repository you want to bring to GDD.
2. Run `gdd init`:

```bash
gdd init
```

3. Answer the Setup form, one step at a time. You should see:

<!-- Real screens of gdd 0.1.0 at 100 columns, blank lines trimmed. -->
```text
┌ gdd init · Step 1 of 7 · Project name ───────────────────────────────────────────────────────────┐
│What is the project called?                                                                       │
│> █                                                                                               │
│Enter: next · Ctrl-C: quit                                                                        │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The form asks for the name, a description, the platform, the stacks (or `Not sure`), the coding
agents, the language the agents use with you, and what would be serious if it failed. The
agents found on your machine come checked, and the first checked one builds:

```text
┌ gdd init · Step 5 of 7 · Coding agents ──────────────────────────────────────────────────────────┐
│Which coding agents? The first checked builds; those found on this machine come checked.          │
│> [x] claude                                                                                      │
│  [x] codex                                                                                       │
│  [x] opencode                                                                                    │
│Up/Down: move · Space: check · Enter: next · Esc: back · Ctrl-C: quit                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

If you want to change an answer, press `Esc`: it goes back one step and keeps what you typed.
Nothing is written until the summary:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Name: notes                                                                                       │
│Description: A command-line notes tool                                                            │
│Platform: cli                                                                                     │
│Stacks: rust                                                                                      │
│Agents: claude, codex, opencode                                                                   │
│Language: en                                                                                      │
│Sensitive: Losing notes                                                                           │
│Enter to set up · Esc to go back                                                                  │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` writes the [harness](../glossary.md#harness), the files gdd uses to check your project,
and commits it (`gdd init 0.1.0`). Then the form runs the first install of each stack, which
writes the lock file (the file that pins the exact version of every dependency), and gdd
commits it too (`gdd bootstrap`). gdd lists every file it wrote and what the install printed.
You should see, for a Python project:

```text
...
.agents/skills/gdd-stack-python/SKILL.md
python: uv lock && uv sync --locked
...
Committed: uv.lock
Next: start your coding agent here and run the gdd-setup command (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

The last line is the one to act on: from here on, you work through your agent. The
[quickstart](../getting-started/quickstart.md) continues from this point.

If the folder already has code, follow [Bring gdd to an existing project](adopt-existing-project.md)
instead.

If the install fails (no network, a missing tool), the harness stays committed and gdd says
what failed. Fix the cause, then run [`gdd bootstrap`](../reference/cli/bootstrap.md) to run
the install again. See [The first install failed](troubleshooting.md#the-first-install-failed).

> [!NOTE]
> If you picked `Not sure` for the technology, the form does not set up the project. It ends
> with the `gdd init` command to run once you know, and says that your coding agent can help you
> choose. No GDD skill is installed yet, so ask your agent in your own words.

A missing tool from step 3 may still go unnoticed until the first change you launch: the
self-test that `/gdd-setup` runs does not run every gate command. If one is missing, it shows
up as a failed gate in that change's Decision Inbox.

## Under the hood

The form gives the same answers as the flags of [`gdd init`](../reference/cli/init.md), which a
script or CI uses instead. [`gdd bootstrap`](../reference/cli/bootstrap.md) runs the first
install of each stack (after `gdd init` with flags, run it yourself or let `/gdd-setup` run
it), [`gdd agents`](../reference/cli/agents.md) writes and checks the skills in each agent's
folder, and [`gdd stacks`](../reference/cli/stacks.md) lists the stack sheets gdd comes with.

## Next steps

- [Quickstart](../getting-started/quickstart.md): from `gdd init` to your first released
  change, talking to your agent.
- [Bring gdd to an existing project](adopt-existing-project.md): when the folder already has
  code.
- [Choose agents and models](agents-and-models.md): which agent and model plays each role.
