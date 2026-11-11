---
name: gdd-prepare
description: Prepares a repository for Gate-Driven Development (GDD) together with the person, from an idea in a sentence or from existing code. Loaded by the GDD orchestrator when there is no blueprint yet, when a change touches paths no zone covers, or when the project changes shape (a new stack, a new delicate area, a new external service). Not for briefing or building a single change.
license: MIT
metadata:
  version: "0.2.1"
  contract: gdd/1
---

# Preparing a project

You decide with the person what the project is about, what is delicate in it, which gates it needs, its constitution and what context each gate carries. You write what each gate checks as scenarios, but you build no checks: `gdd-checks` builds them, and `gdd-review` tries to fool them. You do it yourself, as the orchestrator, with the person, and it needs the deep level, because a wrong blueprint makes every change after it wrong.

## When the project is already prepared

If you're here because a change touches a path no zone covers, or because the project changed shape, do only this. Propose the pathspec or the zone, with its color and why. The person approves that change to the blueprint, and it has to be on the main branch before the change is briefed again, because the color is computed from there. If the zone is new, write its gates, and they get built and tried before any change starts in it. The probe and the baseline aren't repeated, unless the agents' environment changed.

## The conversation

- **Propose first, ask afterwards.** "For a project like this, these are the usual zones and gates; correct what doesn't fit." A blank page makes the person design the method for you.
- **Ask only what changes a zone or a gate:** money, accounts and permissions, personal or regulated data, the stack, what can't be undone. When the person doesn't know, propose what is usual in their sector and say it's a proposal.
- **Ask for external references** that would help a gate: designs, MCP servers, documentation, templates, API specifications, brand guides. Copy each one into `gdd/context/` with its source and date (`assets/context.md`), and write in the blueprint which zones or gates get it, so it doesn't become cross-cutting by accident. An MCP server gets the least access its gate needs.
- **Research** similar products and the sector's rules only if it helps the person decide: `references/research.md`.
- **Ask about emergencies before one happens:** what an urgent change may skip, who opens that gate and what it may never skip. It goes in the blueprint, and the change goes through what it skipped afterwards.

## What's already there

- An existing repository starts with a complete analysis, a baseline of what already fails, and code mapped to zones: `references/existing-project.md`.
- An empty one starts from the idea: `references/new-project.md`.

## Zones

A zone is a set of git pathspecs with a color: green (low), yellow (medium) or red (high), by how much damage a change there can do.

- Mark as delicate whatever is expensive if it fails. In software, start with payments, access and permissions, deployments and CI, the updater, customer data, and whatever judges the rest: the blueprint itself, the checks of the standing gates and what runs the tests.
- Mark behaviors, not only folders: code outside a delicate zone that configures it, feeds it data or decides what it does belongs to it too.
- What can't be undone is never green.
- A zone's tests belong to it: list their paths too. Then propose a catch-all zone for what's left, with the color the person chooses, because anything no zone covers counts as red until the person classifies it.
- Functions don't go in the blueprint. They change often and zones rarely do, so they go to the backlog, each with its expected color.

## Gates

Few gates, but strict: each one costs something on every change, and you'll add another when something slips through. Propose two in every project, whatever it is: no secret reaches the repository, and no new dependency comes in without a person seeing it.

Write the standing gates of each zone in `gdd/gates/<zone>.feature` (`assets/gates.feature`): one `Rule:` for each thing a gate demonstrates, and under it the bad case its check must reject, then one it must accept, each tagged with the id of its check. If you can't get to the `Then`, it isn't a gate yet. Don't wrap a tool in a scenario: "Then the linter reports no errors" tells nobody anything, so the linter goes straight into the blueprint's checks.

```gherkin
@zone:config
Feature: Configuration gates

  Rule: Secrets never reach the repository

    @check:no-secrets
    Scenario: A change that writes an API key into a config file is rejected
      Given a change that adds an API key to config/app.yml
      When the gate runs
      Then the no-secrets check fails and names the file and the line

    @check:no-secrets
    Scenario: A change that reads the key from the environment passes
      Given a change that reads the API key from an environment variable
      When the gate runs
      Then the no-secrets check passes
```

For every check, climb this ladder and stop at the first step that works, because a deterministic check costs no tokens and always gives the same answer:

1. what the stack already brings: types, a linter rule, the test runner;
2. a mature open-source tool with a solid community or company behind it;
3. a small script of the project's own, only when nothing exists (`gdd-checks` writes it, with its rules);
4. a new or less proven tool, only with a notice (how old it is, who maintains it, its license, what it does with the data), and the person decides;
5. a reviewer or a person, when nothing deterministic works.

What each color asks for goes in the blueprint's `colors`, and the template carries the method's proportion. Every color carries `seal`, the check that nothing that judges was touched, with the command the template gives. Where a person approves is the team's decision as well: propose `brief-approval` on yellow and red, so that a person approves what has to pass before anyone builds, and `person-review` on red. Say it's a proposal, and let the team add or remove them on any color. Hidden examples join red only when the blueprint's `guarantee` is `platform` or `engine`, and then the blueprint's `hidden` says where they live.

## What you leave

| File | From | Notes |
|---|---|---|
| `gdd/constitution.md` | `assets/constitution.md` | The stack, the exact commands to build, test and lint, which every gate and CI run as they are, and how a change ships. Only the rules that hold everywhere: a rule of one zone goes to that zone's context in the blueprint. |
| `gdd/blueprint.md` | `assets/blueprint.md` | Levels, limits, checks, colors, zones and context, with its diagram and what an emergency may skip. It's the source: a page for people can be generated from it, never the other way round. |
| `gdd/gates/<zone>.feature` | `assets/gates.feature` | One per zone with standing gates. |
| `gdd/backlog.md` | `assets/backlog.md` | Its first items set up CI where there is none, and then build each zone's gates. `gdd-checks` writes CI as it writes the gates, because the agent that builds doesn't set up what judges it, and until CI runs the blueprint's `guarantee` is `rule`. |
| `gdd/debt.md` | `assets/debt.md` | Only in an existing project with known debt. |
| GDD block of `AGENTS.md` | `assets/agents-block.md` | Under 100 words. If there is a `CLAUDE.md`, add `@AGENTS.md` to it. |

Then the protections. Write a `CODEOWNERS` file that asks for the person's review on what the team always approves (`gdd/blueprint.md`, `gdd/gates/`, `gdd/exam.md`, `gdd/constitution.md` and `gdd/debt.md`), on the agents' own files (`AGENTS.md`, `CLAUDE.md`, skills and settings) and on the zones they mark red. Add `features/` if the team wants every change to what has to pass to reach a person. Leave `gdd/log/`, `gdd/briefs/` and `gdd/backlog.md` out: agents write there on every change. Give the person the rest to apply on the platform, because it takes their credentials: a protected main branch that requires the checks to pass, a rule that keeps `gdd-seal/*` out of the builder's reach, and an agent confined to its work, with no access to production, its own credentials, a restricted network and none of its environment's apps, plugins or connectors that act on the person's accounts, such as publishing a site, sending messages or creating repositories.

## The probe

A cage counts once you've seen it close. When the protections are in place, try four things from the environment the builder will work in, and watch each one fail. Try each with something harmless you can remove, such as an empty commit or a ref named `gdd-seal/probe`, and if a try succeeds, undo it and tell the person. If you can't work from that environment, ask the person to run them there.

1. push to the main branch;
2. write to a `gdd-seal/` ref;
3. read the hidden examples, if the project has them (when a team adds them later, this try runs again);
4. find a credential of production or of the person within reach (an environment variable, a logged-in command line, a connector). Finding it is enough: never use it.

Record each try in `gdd/log/blueprint.jsonl`, with its command, its exit code and its last lines. If all of them fail, the blueprint's `guarantee` is `platform`. If any succeeds, it's `rule`: say so to the person, write it in the blueprint and leave hidden examples out of red, because a builder that can read them isn't being tested by them.

## The baseline and the limits

Before the first change, write the baseline in the blueprint with the person, from the project's own history: how long a change takes from request to release, how often it stops someone and what slips through. A new project has no history: say so there, and take the baseline from its first changes. Then ask what figure would make the team drop a gate, and write it under it. Without both, "it's going better" is only an impression.

Set the caps of a change in `limits` with the person too. The template's values are provisional: keep them only until the project's own logs say otherwise. The cap on tokens has no default, and makes sense only if the agents' environment reports them.

## Approval

The person approves the first blueprint, and the approval stays written: a commit or a pull request of theirs. Recording known debt loosens a check, so it's part of that same approval. Then each zone's gates get built and tried, and no change starts in a zone until its gates hold.

## Gotchas

- In the T-minus pilot, its own gate passed while the project's linter failed: the gate didn't run the same lint. Every gate runs the constitution's commands, the same ones as CI.
- Eighteen checks already failed on the main branch before anyone touched anything, fourteen scenarios and four unit tests. Without a baseline, every change gets blamed for them, or they quietly disappear.
- Some checks can't run inside the agent's box (the pilot's self-test couldn't open a file it needed inside the sandbox). Say which checks run outside it, and who runs them.
- An agent's environment can act on the person's accounts. In one run, an agent published the project on the person's own account through a site-publishing plugin of its environment; in another, it used the person's git credentials to create repositories and try to deploy. Confine the environment, not only the project.

When you close, write the `prepare` step to `gdd/log/blueprint.jsonl`: its `result` is `proposed` or `approved`.
