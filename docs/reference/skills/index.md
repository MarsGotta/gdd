# Skills

A gdd skill is a set of instructions that `gdd init` installs into your coding agent. You run
a skill in a conversation with your agent, and the agent follows it step by step, running the
gdd commands for you. This page lists every skill, where it lives and how you call it.
Consult it to find the skill for the step you are at.

## Before you start

- A project set up with [`gdd init`](../cli/init.md). It installs the skills for every coding
  agent you picked.
- Your coding agent, started in the project folder, on `main`.

## Where the skills live

Each agent keeps the skills in its own folder, under the names `gdd-<name>`:

| Agent | Files `gdd init` writes | How you call a skill |
|---|---|---|
| Claude Code | `.claude/skills/gdd-<name>/SKILL.md` | `/gdd-change` |
| Codex | `.agents/skills/gdd-<name>/SKILL.md` | `$gdd-change` |
| OpenCode | `.opencode/commands/gdd-<name>.md` | `/gdd-change` |

Every file starts with a line that says it is generated: edits there are lost.
[`gdd agents install`](../cli/agents-install.md) writes them again, for example after you
change `gdd/models.toml` or upgrade gdd, and [`gdd agents check`](../cli/agents-check.md) says
whether one was edited or is missing.

The folders belong to the [harness](../../glossary.md#harness), the files gdd uses to check
your project, which is a [red zone](../../glossary.md#red-zone): review those files and commit
them on `main`.
[Choose agents and models](../../how-to/agents-and-models.md#5-install-the-gdd-commands-in-each-agent)
shows both commands with their output.

## How you call a skill

Type its name in your agent, followed by what you want in your own words. The words after
the name are the skill's input. In Claude Code and OpenCode:

```text
/gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

In Codex:

```text
$gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

The input is optional for every skill: `/gdd-status` alone is a complete call. The agent plays
the [Orchestrator](../roles.md#the-orchestrator) and talks to you in the language of
`language` in `gdd/project.toml`. The skills point to each other in the agent's own syntax:
in Codex, `gdd-status` proposes `$gdd-change`; in Claude Code, `/gdd-change`.

## The skills you run

| Skill | Run it to | When |
|---|---|---|
| [`gdd-setup`](setup.md) | write the Project Brief, pick the scenario runner and leave the harness ready, so the machines open the Setup Gate (G0) | once per project, right after `gdd init` |
| [`gdd-discover`](discover.md) | explore the next piece of value with you: options, spikes, decisions and the order of the next changes | after Setup, and whenever the next step is not one small change |
| [`gdd-change`](change.md) | prepare one change, ask for your OK at one sitting, launch it and bring it back at the Release Gate (G4) | every time you want something built, fixed or reshaped |
| [`gdd-status`](status.md) | hear where the project stands and the one next step | when you come back to a project, or are unsure what comes next |
| [`gdd-report`](report.md) | read what the agent calls cost, in tokens and money | when you want the numbers |
| [`gdd-learn`](learn.md) | find errors that repeat and propose one rule or check for each | from time to time, after several changes |
| [`gdd-hotfix`](hotfix.md) | roll production back first, then fix the cause as a change of its own | when something is broken in production |

The usual order is `gdd-setup`, then `gdd-change` for each change. Run `gdd-discover`
before a change when the product first needs an architecture or a map of its sensitive areas.
`gdd-status` is the way back in at any point: it always ends with one next step.

## The skills gdd's agents use

When gdd runs a change, it launches agents itself, without a conversation, and hands each one
of these skills. You never run them: they expect an input that only gdd writes.

| Skill | Who gets it | What it does |
|---|---|---|
| [`gdd-build-test`](agent-skills.md#gdd-build-test) | the Builder in test mode | writes the scenario steps and the tests, which fail because the behavior is missing |
| [`gdd-build`](agent-skills.md#gdd-build) | the Builder in build mode | writes the code until the Loop Gate (G2) passes |
| [`gdd-review`](agent-skills.md#gdd-review) | the Reviewer | reads the whole change and answers OK or KO |
| [`gdd-replan`](agent-skills.md#gdd-replan) | the Orchestrator, headless | finds why G2 is still red after the Builder's retries and writes a new approach |

gdd also writes one `gdd-stack-<name>` skill for each stack in `gdd/project.toml`, such as
`gdd-stack-python`: the stack's own instructions, which those agents read before they work.
[Skills of gdd's agents](agent-skills.md) has the details.

## See also

- [Describe a change to your agent](../../how-to/run-a-change.md): the change skill in use.
- [Roles](../roles.md): what the Orchestrator, the Builder and the Reviewer may and may not do.
- [CLI reference](../cli/index.md): the gdd commands the skills run.
- [Screens](../screens.md): the sitting and the Board, if you prefer a screen to the chat.
