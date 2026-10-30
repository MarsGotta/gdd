# Install gdd

Install the `gdd` binary, at least one coding agent it can drive, and the tools the gates of
your stack run. At the end you check that all three are in place.

## Before you start

- Linux or macOS. gdd runs its gates through `sh` and the agents in git worktrees.
- [git](https://git-scm.com).
- [Rust](https://rustup.rs) 1.98 or newer, to build gdd. `rustup update` brings an older
  toolchain up to date.

gdd 0.1.0 installs with cargo only. Prebuilt binaries, crates.io and Homebrew are on the
[roadmap](../../ROADMAP.md#5-open-source) and not available yet.

## 1. Install the binary

Install the tagged version from GitHub. This is the command the CI workflow that
[`gdd init`](../reference/cli.md#gdd-init) writes runs too:

```bash
cargo install --locked --git https://github.com/MarsGotta/gdd --tag v0.1.0 gdd
```

cargo builds gdd and puts it in `~/.cargo/bin`, which rustup adds to your `PATH`.

> [!NOTE]
> The workflow `gdd init` writes installs gdd from the tag `v<version>`, so CI works once
> that tag exists on GitHub. If cargo says `failed to find tag v0.1.0`, the tag is not
> published yet: install from the main branch with the same command without `--tag v0.1.0`.

### Or build from source

Clone the repository and install from the checkout:

```bash
git clone https://github.com/MarsGotta/gdd
cd gdd
cargo install --locked --path crates/gdd
```

The repository pins its toolchain in `rust-toolchain.toml`, so rustup downloads Rust 1.98.1
the first time you build there. To work on gdd itself, follow
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

In a project, gdd is always called through `bin/gdd`, a short script `gdd init` writes. It
runs the `gdd` on your `PATH`, or the binary the variable `GDD_BIN` names:

```bash
GDD_BIN=/home/me/src/gdd/target/release/gdd bin/gdd status
```

## 2. Install a coding agent

gdd does not talk to a model itself. It starts the command-line program of each coding agent,
headless, with the arguments of the role it plays. It knows three agents, one data sheet
each:

| Agent | Program on the `PATH` | Commands written to | Called as |
|---|---|---|---|
| [Claude Code](https://code.claude.com/docs) | `claude` | `.claude/skills/` | `/gdd-status` |
| [Codex CLI](https://developers.openai.com/codex) | `codex` | `.agents/skills/` | `$gdd-status` |
| [OpenCode](https://opencode.ai/docs) | `opencode` | `.opencode/commands/` | `/gdd-status` |

Install the ones you want with their own instructions and sign in to each once, interactively.
gdd uses those logins (a subscription works). It never hands your API keys to an agent: it
removes from the agent's environment every variable with a word such as `KEY`, `TOKEN`,
`SECRET` or `PASSWORD` in its name, and every URL with a password in it, unless `[env] pass`
in `gdd/models.toml` names the variable on purpose.
[Environment variables](../reference/environment.md#which-variables-hold-a-secret) has the full
rule.

Install agents of **two model families**. The Reviewer must be of another family than the
Builder, so it does not share its blind spots: for example Claude Code to build and Codex to
review. With one family only, `gdd init` refuses and says what to do:

```text
error: one model family only (anthropic): the Reviewer would share the Builder's blind spots. Install an agent of another family (OpenCode takes models from many providers) or run again with --accept-same-family.
```

`gdd init --agents` names the agents of a project, the first one building. Without the flag,
init takes those it finds on the `PATH`. [Choose agents and models](agents-and-models.md) says
how to change them later.

## 3. Install the tools of your stack

Each stack sheet names the commands its gates run. Some tools come with the project's own
dependencies, installed by [`gdd bootstrap`](../reference/cli.md#gdd-bootstrap); the others
must be on your `PATH`.

### Python

| Need | Tool | How it gets there |
|---|---|---|
| Environment, every command | [uv](https://docs.astral.sh/uv/) | install it yourself |
| Lint, types, unit tests, scenarios, architecture, security, mutation, unused code | Ruff, mypy, pytest, pytest-bdd, import-linter, Bandit, mutmut, vulture | dev dependencies in `pyproject.toml`, installed by `gdd bootstrap` |
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

Mutation testing on Rust links with [mold](https://github.com/rui314/mold) when it is
installed, and caps each test process with `prlimit` where it exists (Linux). Both are
optional.

### TypeScript

| Need | Tool | How it gets there |
|---|---|---|
| Every command | Node.js and npm | install them yourself |
| Lint, types, unit tests, scenarios, architecture, mutation | ESLint, tsc, Vitest, cucumber-js, dependency-cruiser, StrykerJS | dev dependencies in `package.json`, installed by `gdd bootstrap` |
| G3 security and licenses | OSV-Scanner | install it yourself |
| G3 secrets | gitleaks | install it yourself |
| Learn: unused code, duplication | knip, jscpd, through `npx` | fetched by `npx` |

`gdd stacks show NAME` prints the full sheet of a stack, with every command its gates run.
The [stacks reference](../reference/stacks.md) lists them all.

## 4. Check the installation

The checks that read a stack sheet run inside a GDD project. Make a throwaway one:

```bash
mkdir notes && cd notes
git init -b main
gdd init --name notes --platform cli --stack python --agents claude codex
bin/gdd bootstrap
```

`gdd init` lists every file it wrote and commits them; `gdd bootstrap` installs the stack's
dependencies and commits the lock file. Then check each piece:

```bash
bin/gdd stacks
bin/gdd agents check
bin/gdd selftest
```

You should see:

```text
python · Python · pytest-bdd
rust · Rust · cucumber-rs
typescript · TypeScript · cucumber-js
{"ok":true}
OK a failing test turns G2 red
OK editing a protected file is blocked (Path Guard and CI)
OK rollback works in pre (simulated)
OK zero scenarios counts as a failure
harness 4/4
```

- `gdd stacks` lists the stack sheets gdd comes with.
- `gdd agents check` says the GDD commands in each agent's folder are the ones gdd writes.
- `gdd selftest` proves the harness works: a failing test turns G2 red, Path Guard blocks a
  protected file, rollback works in pre and zero scenarios fail G3. Anything short of
  `harness 4/4` names the check that failed.

The self-test does not run every gate command. The first change you launch is the real test
of the tools in step 3: a missing one shows up as a failed gate in its Decision Inbox.

## Next steps

- [Your first change](../getting-started/tutorial.md): set up a project and take one change
  from scenario to release.
- [Bring gdd to an existing project](adopt-existing-project.md): when the folder already has
  code.
- [Choose agents and models](agents-and-models.md): which agent and model plays each role.
