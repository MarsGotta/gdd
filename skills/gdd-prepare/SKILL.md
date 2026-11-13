---
name: gdd-prepare
description: Prepares a repository for Gate-Driven Development (GDD) with the person, from an idea in a sentence or from existing code. Loaded by the GDD orchestrator (`gdd`, which loads first) when there is no blueprint yet, when a change touches paths no zone covers, or when the project changes shape (a new stack, a new delicate area, a new external service). Not for briefing or building a single change.
license: MIT
metadata:
  version: "0.10.0"
  contract: gdd/1
---

# Preparing a project

You decide with the person what the project is about, what is delicate in it, which gates it needs, its constitution and what context each gate carries. You build no checks: `gdd-checks` sets them up, and `gdd-review` tries them. You do it yourself, as the orchestrator, with the person: if the `gdd` skill isn't loaded, load it now, because the shapes of the log are there. It needs the deep level, because a wrong blueprint makes every change after it wrong.

## When the project is already prepared

If you're here because the blueprint fell short, do only this: a path no zone covers, which `gdd-brief` judged and left in the backlog, or a project that changed shape. Propose the pathspec or the zone, with its color and why. The person approves that change to the blueprint, and it has to be on the main branch before the next change is briefed, because the color is computed from there. A new zone gets a color and no gates of its own. If that color asks for a check the project hasn't set up yet, it gets set up and tried before any change starts there, as the backlog item `standing-checks-<check id>`. The probe and the baseline aren't repeated, unless the agents' environment changed. If you're here because an environment or a model of `levels` has to change, do only `references/levels.md`.

## The conversation

- **Propose first, ask afterwards.** "For a project like this, these are the usual zones and gates; correct what doesn't fit." A blank page makes the person design the method for you.
- **Ask only what changes a zone or a gate:** money, accounts and permissions, personal or regulated data, the stack, what can't be undone. When the person doesn't know, propose what is usual in their sector and say it's a proposal.
- **Ask for external references** that would help a gate: designs, MCP servers, documentation, templates, API specifications. Copy each one into `gdd/context/` with its source and date (`assets/context.md`), and write in the blueprint which zones or gates get it, so it doesn't become cross-cutting by accident. An MCP server gets the least access its gate needs.
- **Research** what is usual in the project's sector before you propose, in a new project or when the person leaves it to you: `references/research.md`.
- **Ask which models do the work.** See which agents' environments the person has and what each one offers, propose a model for each level, with the reviewer on the same family unless another was seen to work, and try each one before it counts: `references/levels.md`.
- **Ask about emergencies before one happens:** what an urgent change may skip, who opens that gate and what it may never skip. It goes in the blueprint, and the change goes through what it skipped afterwards.
- **Ask what the gates may cost them:** how long they'd wait for a change, and how often they want to be stopped to approve something. The first sets the `limits`, the second which colors carry an approval. If they leave it to you, keep the template's.

## What's already there

- An existing repository starts with a complete analysis, a baseline of what already fails, and code mapped to zones, which you try on its history and report, before the person approves: `references/existing-project.md`.
- An empty one, or one with no code yet (only data or documents), starts from the idea: `references/new-project.md`.

## Zones

A zone is a set of git pathspecs with a color: green (low), yellow (medium) or red (high), by how much damage a change there can do.

- Mark as delicate whatever is expensive if it fails. In software, start with payments, access and permissions, deployments and CI, the updater, customer data, and whatever judges the rest, in a zone whose color asks for `review`: the blueprint itself, the standing checks and what runs them (CI and each tool's configuration).
- Mark behaviors, not only folders: code outside a delicate zone that configures it, feeds it data or decides what it does belongs to it too.
- What can't be undone is never green.
- List the dependency manifests and their lock files in a zone whose color asks for `person-review`. That is all it takes for no new dependency to come in without a person seeing it: there is no check to write.
- A zone's tests belong to it: list their paths too. Then ask about what's left: a catch-all zone with the color the person chooses, or none, and then each change that touches it is judged, and is never green.
- Functions don't go in the blueprint. They change often and zones rarely do, so they go to the backlog, each with its expected color.

## Gates

Which gates the project needs is the person's decision, and they differ with the discipline and the project. Before you propose any, open `references/gates.md`: what a project starts with, what comes later and how to read the gates it already has. When it builds for the web, open `references/web.md` too. Propose the usual minimum, each gate with what it has to demonstrate and the checks that demonstrate it, and each check with its tool's own numbers, never the strictest: what is stricter earns its place when hardening, with what slipped through as its evidence. A check that not every color asks judges only what the change adds or changes: its command takes `<start>`, like `seal`.

The gates are the project's, the same for every change. A zone gets none of its own when the project is prepared: it gives a change its color, and the color says how much each gate asks, so more risk means more checks, not other gates.

Their checks are standing checks, which hold for the whole project. List them once in the blueprint's `checks`, each with the command the constitution gives it, what breaks it (`breaks`) and, when it applies only to some paths, its `when`. For software they are usually the tests, the linter and a scan that keeps secrets out of the repository (the template's `secrets`), and that scan you propose in every project. Write each gate as a feature, `gdd/gates/<gate>.feature` (`assets/gate.feature`): which checks it asks of each color, and the bad case each check rejects. It is the gate's specification and the blueprint is what runs, so name it in `gates`. Write none for a zone on your own: a rule of one zone comes with the change that first needs it, or from hardening. If the person asks for one now, write it in `gdd/gates/<zone>.feature` (`assets/gates.feature`), and it gets built and tried as `gates-<zone>`.

Each check says when it runs (`runs`): `each round`, within the two minutes the template gives those together, or `before review`, once the rest pass, for what takes longer, so a fix round doesn't pay for it. Put them in the blueprint's table of checks, in the person's words, for one yes. For every check, climb the ladder in `references/gates.md` and stop at the first step that works.

What each color asks for goes in the blueprint's `colors`, and the template carries the method's proportion. Every color carries `seal`, the check that nothing that judges was touched, with the command the template gives. Where a person approves is the team's decision as well: propose `brief-approval` and `person-review` on red, so that a person approves what has to pass before anyone builds and the change at the end. On yellow the brief is still written and nobody waits for it. Say it's a proposal: they, or their team, can add or remove them on any color. Hidden examples join red only when the blueprint's `guarantee` is `platform` or `engine`, and then the blueprint's `hidden` says where they live.

## What you leave

| File | From | Notes |
|---|---|---|
| `gdd/constitution.md` | `assets/constitution.md` | The stack, which is the one the request names when it names one, the exact commands to build, test, lint and scan for secrets, which every gate and CI run as they are, and how a change ships. Then what must never happen in the product, whichever part a change touches (in a shop: nothing sold without stock, a paid order never changes), from its data and what you looked up, in the person's words, three to seven, for them to correct: every brief is checked against them, and a rule left in a zone's context is never seen by a change outside it. How one zone's code is written goes to its context. |
| `gdd/blueprint.md` | `assets/blueprint.md` | Levels, limits, checks, colors, zones and context, with its diagram and what an emergency may skip. Each level is a model by the name the agents' environment launches it with, never a description of how much it reasons. |
| `gdd/check.sh`, `gdd/limits.sh` | `assets/check.sh`, `assets/limits.sh` | A runner that times each check, unless the person says no: the constitution's commands go through it, `seal` apart. And the counter of what `limits` caps, which the orchestrator runs before each round. |
| `gdd/gates/<gate>.feature` | `assets/gate.feature` | One for each gate of the project. A zone gets one (`assets/gates.feature`) only if the person asked for it. |
| `gdd/backlog.md` | `assets/backlog.md` | Its first item, `standing-checks`, sets up what the blueprint's `checks` name, and CI where there is none. `gdd-checks` does it, because the agent that builds doesn't set up what judges it, and until CI runs the blueprint's `guarantee` is `rule`. |
| `gdd/debt.md` | `assets/debt.md` | Only in an existing project with known debt. |
| GDD block of `AGENTS.md` | `assets/agents-block.md` | Under 100 words. If there is a `CLAUDE.md`, add `@AGENTS.md` to it. |

Then the protections. Write a `CODEOWNERS` file that asks for the person's review on what the team always approves (`gdd/blueprint.md`, `gdd/gates/`, `gdd/check.sh`, `gdd/limits.sh`, `gdd/exam.md`, `gdd/constitution.md` and `gdd/debt.md`), on the agents' own files (`AGENTS.md`, `CLAUDE.md`, skills and settings) and on the zones they mark red. Add `features/` if the team wants every change to what has to pass to reach a person. Leave `gdd/log/`, `gdd/briefs/` and `gdd/backlog.md` out: agents write there on every change. Give the person the rest to apply on the platform, because it takes their credentials: a protected main branch that requires the checks to pass, a rule that keeps `gdd-seal/*` out of the builder's reach, and an agent confined to its work, with no access to production, its own credentials, a restricted network and none of its environment's apps, plugins or connectors that act on the person's accounts.

## The probe

A cage counts once you've seen it close. When the protections are in place, try four things from each environment in `levels`, in a role launched the way its `launch` says, and watch each one fail. Try each with something harmless you can remove, such as an empty commit or a ref named `gdd-seal/probe`, and if a try succeeds, undo it and tell the person. If you can't work from that environment, ask the person to run them there.

1. push to the main branch;
2. write to a `gdd-seal/` ref;
3. read the hidden examples, if the project has them (when a team adds them later, this try runs again);
4. find a credential of production or of the person within reach (an environment variable, a logged-in command line, a connector). Finding it is enough: never use it.

Record each try in `gdd/log/blueprint.jsonl`, with its command, its exit code and its last lines. If all of them fail in every one, the blueprint's `guarantee` is `platform`. If any succeeds, it's `rule`: say so to the person, write it in the blueprint and leave hidden examples out of red, because a builder that can read them isn't being tested by them.

## The baseline and the limits

Before the first change, write the baseline in the blueprint with the person, from the project's own history: how long a change takes from request to release, how often it stops someone and what slips through. A new project has no history: say so there, and take the baseline from its first changes. Then ask what figure would make the team drop a gate, and write it under it.

Set the caps of a change of each color, in its `limits`, from how long the person said they'd wait. The template's values are a start: keep them only until the project's own logs say otherwise.

## Approval

The person approves the first blueprint, and the approval stays written: a commit or a pull request of theirs, or their yes in this conversation where there's no platform. They approve the files as they are, so write every change into them before you ask, and show them the files, not a summary: whatever you change after their yes goes back to them. Ask with the message in `assets/approval.md`: copy it, fill it in and send nothing else, because what you decided is in the files. Recording known debt loosens a check, so it's part of that same approval. Then the standing checks get set up and tried, once for the whole project, and no change starts until they hold. Tell the person what their trial leaves under `LATER:` and put it in the backlog, without waiting: it holds no change back, and what they approved stays as it is unless they change it. An agent never widens what has to pass, not even when the person leaves it to you.

## Gotchas

- Eighteen checks already failed on the main branch before anyone touched anything. Without a baseline, every change gets blamed for them, or they quietly disappear.
- Some checks can't run inside the agent's box (the pilot's self-test couldn't open a file there). Say which checks run outside it, and who runs them.
- In one run, an agent published the project on the person's own account through a plugin of its environment; in another, it used the person's git credentials to create repositories. Confine the environment, not only the project.

When you close, write the `prepare` step to `gdd/log/blueprint.jsonl`: its `result` is `proposed` or `approved`.
