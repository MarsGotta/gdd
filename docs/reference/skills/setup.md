# gdd-setup

The setup skill is Phase 0 of GDD. The Orchestrator asks you a few basic questions, writes the
Project Brief and leaves the harness ready, so the machines open the Setup Gate (G0). You run
it once per project, always first, right after `gdd init`. The harness is the set of files gdd
uses to check your project.

## Before you start

- A project made by [`gdd init`](../cli/init.md). The form also runs the first install of each
  stack, and ends by telling you to run this skill. After `gdd init` with flags, the skill runs
  that install itself.
- The project may be new or already have code. In a repository with code, the skill records
  what fails today as known debt (step 8).

## Call it

In Claude Code and OpenCode:

```text
/gdd-setup
```

In Codex:

```text
$gdd-setup
```

The input is optional. Use it to say what you already know, so the questions are fewer:

```text
/gdd-setup a command-line notes tool for one person; losing a note would be serious
```

## What it asks you

First it reads what is already there: `gdd/project.toml` and `docs/brief.md`, which
`gdd init` wrote from your answers to the form, then any seed or idea document in the
repository (a `SEED.md`, for example) and the code, when the repository is not empty. Then it
asks, in one short message, only what those files leave out. A name, a platform and a stack in
`gdd/project.toml` are not asked again; an empty section of the Brief is. The questions it may
ask:

- What is the project, in five lines? When a seed document exists, it summarizes it and asks
  you to confirm instead.
- Where does it run: mobile, desktop, web, command line or other?
- Is the technology decided? If not, it recommends one option that fits the platform, with the
  reason in two lines, and at most one alternative.
- What would be serious if it failed: losing data, privacy, money, production, keys?

Before it ends, it asks one more thing: your OK on the red zones `gdd init` wrote (step 6).

Every question comes with its recommendation first. When something is reversible and it
hesitates between asking and deciding, it decides and tells you. Its last message ends with one
clear next step.

## What it does

1. **Writes the Project Brief** in `docs/brief.md`: the idea, who it is for, the platform, the
   technology and why, the sensitive areas, the constraints, the minimum viable product (MVP)
   and the kill rule when they exist, and the open questions. It fills `name`, `platform` and
   `stack` in `gdd/project.toml` and records the technology choice as an architecture decision
   record (ADR) in `docs/adr/0001-technology.md`.
2. **Picks the scenario runner.** GDD requires Gherkin, the plain-language format of the
   scenarios, not a tool to run it. The Orchestrator recommends only runners with broad
   support, such as the official Cucumber implementation for the language or the standard of
   its ecosystem. Examples are cucumber-js for TypeScript, pytest-bdd or behave for Python and
   cucumber-rs for Rust. It checks that the runner is still maintained, fails on undefined or
   pending steps, runs a subset of `.feature` files and runs `features/hidden/`. The choice
   goes in the Brief and in `scenario_runner` of `gdd/project.toml`.
3. **Completes the skeleton** in that technology: the folders, one unit test, a first smoke
   scenario in `features/` with its step, and the folders `features/regression/` and
   `features/hidden/`. Steps talk to the core of the application, not to the screen.
4. **Writes the gate commands** in `gdd/commands.toml`: what the Loop Gate (G2), the Merge
   Gate (G3), the release steps and the self-test run. It tries each command by hand before it
   writes it. With no real pre or production yet, it sets `simulated = true` and tells you.
5. **Completes the continuous integration (CI) workflow** `.github/workflows/gdd.yml` with
   the setup of the technology.
   CI runs [`gdd ci`](../cli/ci.md).
6. **Asks you to approve the red zones `gdd init` wrote.** Your sensitive areas stay as words
   in the Brief, since there are no real folders to map yet, and unmapped code is yellow. The
   zone map holds only the zones `gdd init` wrote, and the harness zone ships red. No change can
   be launched before a person approved the zone map, so the skill asks you in one line, saying
   what the harness zone protects: gdd's own files, the CI and the agent instructions. With your
   OK it runs [`gdd gate zones ok --by human`](../cli/gate.md). The discover skill maps your own
   sensitive areas later.
7. **Protects the right paths** in `gdd/rules.toml`: the production code against the Builder
   in test mode, and the folder of the scenario steps against the Builder in build mode.
8. **Installs and adopts when needed.** When a stack's lock file (the file that pins the
   exact versions of its dependencies) is missing after `gdd init` with flags, it runs
   [`gdd bootstrap`](../cli/bootstrap.md). When the repository already had code, it runs
   [`gdd adopt`](../cli/adopt.md) before the self-test:
   what fails today becomes known debt in `gdd/debt.toml`, so G0 can open, and it tells you in
   two lines what became debt and that each debt gets a Refactor.
9. **Runs the self-test**, [`gdd selftest`](../cli/selftest.md), until it says
   `harness 4/4`.
10. **Opens G0 as the machines**, with [`gdd gate G0 ok --by machines`](../cli/gate.md). G0
    opens only with the Brief written and the self-test at 4/4. It tells you in two lines that
    the harness is ready.
11. **Commits on `main`** with the message `gdd: setup (G0)`.
12. **Counts the conversation** with [`gdd session --phase 0-setup`](../cli/session.md), which
    adds the requests of your interactive Claude Code sessions to the log.

## What you decide

- Your answers to the questions, and whether to take each recommendation: the technology and
  the scenario runner above all.
- The red zones `gdd init` wrote, with one OK.
- Nothing about G0 itself. The machines open it when the Brief exists and the self-test
  passes; it is not a human stop. See [G0 Setup Gate](../gates.md#g0-setup-gate).

## What it leaves behind

| What | Where |
|---|---|
| The Project Brief | `docs/brief.md` |
| The technology decision | `docs/adr/0001-technology.md` |
| Name, platform, stacks and scenario runner | `gdd/project.toml` |
| The gate and release commands | `gdd/commands.toml` |
| The protected paths | `gdd/rules.toml` |
| The skeleton and the first smoke scenario | the stack's folders, `features/` |
| The CI workflow | `.github/workflows/gdd.yml` |
| The known debt, in a project that had code | `gdd/debt.toml`, in its own commit (`gdd adopt: known debt in ...`) |
| Your approval of the red zones | `.gdd/zones.json` |
| The self-test result and G0 | `.gdd/selftest.json`, `.gdd/g0.json`, `.gdd/log.jsonl` |
| The setup commit | `gdd: setup (G0)` on `main` |

[Files](../files.md) describes each file.

## What comes next

The skill ends by sending you to the first change: run [`gdd-change`](change.md) with what
you want, in one sentence. When the product first needs an architecture or a map of its
sensitive areas, it proposes [`gdd-discover`](discover.md) first.
[`gdd status`](../cli/status.md) now says `G0: ok`.

## See also

- [Bring gdd to an existing project](../../how-to/adopt-existing-project.md): Setup on a
  repository that already has code.
- [Gates](../gates.md#g0-setup-gate): what G0 checks and when it says no.
- [Stacks](../stacks.md): what each stack sheet brings to the skeleton and the gate commands.
