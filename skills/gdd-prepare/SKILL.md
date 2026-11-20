---
name: gdd-prepare
description: Prepares a repository for Gate-Driven Development (GDD) with the person, from an idea in a sentence or from existing code. Loaded by the GDD orchestrator (`gdd`, which loads first) when there is no blueprint yet, when a change touches paths no zone covers, or when the project changes shape (a new stack, a new delicate area, a new external service). Not for briefing or building a single change.
license: MIT
metadata:
  version: "0.19.0"
  contract: gdd/1
---

# Preparing a project

As orchestrator, prepare the project with the person and the `gdd` skill loaded. Agree on its purpose, delicate areas, gates, constitution and context for each gate. Use the deep level because a wrong blueprint affects every change. The checks belong to `gdd-checks`, and `gdd-review` tries them.

## When the project is already prepared

If you're here because the blueprint fell short, do only this: a path no zone covers, which `gdd-brief` judged and left in the backlog, or a project that changed shape. Propose the pathspec or the zone, with its color and why. The person approves that change to the blueprint, and it has to be on the main branch before the next change is briefed, because the color is computed from there. For a new red zone, propose its rules as below. If that color asks for a check the project hasn't set up yet, it gets set up and tried before any change starts there, as the backlog item `standing-checks-<check id>`. The probe and the baseline aren't repeated, unless the agents' environment changed. If you're here because an environment or a model of `levels` has to change, do only `references/levels.md`.

## The conversation

- **Propose first, ask afterwards.** "For a project like this, these are the usual zones and gates; correct what doesn't fit." A blank page makes the person design the method for you.
- **Ask only what changes a zone or a gate:** money, accounts and permissions, personal or regulated data, the stack, what can't be undone. When the person doesn't know, propose what is usual in their sector and say it's a proposal.
- **Ask for external references** that would help a gate: designs, MCP servers, documentation, templates, API specs. Copy each one into `gdd/context/` with its source and date (`assets/context.md`), and write in the blueprint which zones or gates get it, so it doesn't become cross-cutting by accident. An MCP server gets the least access its gate needs.
- **Research** before you propose, in a new project or when the person leaves it to you: open `references/research.md` and look up what is usual in the sector.
- **Open each reference by itself:** several in one command come back cut.
- **Ask which models do the work:** propose one for each level, from what each of the person's environments offers, and try each before it counts, as `references/levels.md` says.
- **Ask about emergencies before one happens:** what an urgent change may skip, who opens that gate and what it may never skip. It goes in the blueprint, and the change goes through what it skipped afterwards.
- **Ask what the gates may cost them:** how long they'd wait for a change, and how often they'd stop to approve something. The first sets the `limits`, the second which colors carry an approval. If they leave it to you, keep the template's.

## What's already there

- For a project with code, follow `references/existing-project.md`.
- For an idea, data or documents without code, follow `references/new-project.md`.

## Zones

A zone is a set of git pathspecs with a color: green (low), yellow (medium) or red (high), by how much damage a change there can do. Try the draft before approval. Change a zone only for a reason grounded in risk, and try it again. Never lower a color to improve the figures.

- Mark as delicate whatever is expensive if it fails. In software, start with handling money, access and permissions, deployments and CI, the updater, personal data, and whatever judges the rest, in a zone whose color asks for `review`: the blueprint itself, the standing checks and what runs them (CI and each tool's configuration).
- Mark behaviors, not only folders: code outside a delicate zone that configures it, feeds it data or decides what it does belongs to it too.
- What can't be undone is never green.
- Propose a green zone for documents that don't change how the product behaves, such as guides and notes, unless the person says otherwise.
- List the dependency manifests and their lock files in a zone whose color asks for `person-review`. That is all it takes for no new dependency to come in without a person seeing it: there is no check to write.
- A zone's tests belong to it. Include their paths. Check each empty pathspec for a typo and try planned filenames where the folder does not exist yet. For files no zone covers, propose a catch-all zone with the person's choice of color, or leave each change to be judged, never green.
- Functions don't go in the blueprint. They change often and zones rarely do, so they go to the backlog, each with its expected color.

## Gates

The person or their team chooses the gates, since they differ with the discipline and the project. The same gates apply to every change. The person also decides what each color asks. More risk means more checks at those gates. Before you propose any, open `references/gates.md`, and `references/web.md` too when it builds for the web. Propose the usual minimum, each gate with what it has to demonstrate and the checks that demonstrate it, and each check with its tool's own numbers, never the strictest: what is stricter earns its place when hardening, with what slipped through as its evidence. A check that not every color asks judges only what the change adds or changes: its command takes `<start>`, like `seal`.

For each red zone, propose three to five things that must never happen, in the person's words. Open `references/research.md` when choosing them from the project's data and what is usual in its sector. The person can approve or correct them with the blueprint. Yellow and green get their own gates only on request or through hardening.

List standing checks once in the blueprint's `checks`, with the constitution's command, the bad case (`breaks`) and any path condition (`when`). In software these usually include tests and lint. Propose a secret scan in every project. Name each project gate's feature in `gates`. It says what each color asks and what each check rejects. Write each zone rule as a `Rule:` in `gdd/gates/<zone>.feature`, with two scenarios: a change that breaks it, which the gate rejects, and one that keeps it. Link that file from the zone's `gates` field. Tag both for the reviewer (`@check:review`) on every change touching the zone, and also for a test or a scan wherever one could observe them, even before that test is written. For rules a test can cover, the orchestrator adds `gates-<zone>` to the backlog during Prepare. Put it after `standing-checks` if the behavior exists, or just after the first item that brings it. There `gdd-checks` writes the tests.

Each check says when it runs (`runs`): `each round`, within the two minutes the template gives those together, or `before review`, for what takes longer, so a fix round doesn't pay for it. Put them in the blueprint's table of checks, in the person's words, for one yes. For every check, climb the ladder in `references/gates.md` and stop at the first step that works.

What each color asks for goes in the blueprint's `colors`, and the template carries the method's proportion. Every color carries `seal`, the check that nothing that judges was touched, with the command the template gives. Where a person approves is the team's decision as well: propose `brief-approval` on yellow and red, for what has to pass before anyone builds, and `person-review` on red, for the change at the end. Say it's a proposal: they, or their team, can add or remove them on any color; without one on yellow, the person is told in a line what the change will do. Hidden examples join red only when the blueprint's `guarantee` is `platform` or `engine`, and then the blueprint's `hidden` says where they live.

## What you leave

| File | From | Notes |
|---|---|---|
| `gdd/constitution.md` | `assets/constitution.md` | The stack, which is the one the request names when it names one, the exact commands to build, test, lint and scan for secrets, which every gate and CI run as they are, and how a change ships. Then what must never happen in the product, whichever part a change touches, from its data, one for each amount, quantity or state its records keep, and what you looked up, in the person's words, three to seven, for them to correct: every brief is checked against them, and a rule left in a zone's context is never seen by a change outside it. How one zone's code is written goes to its context. |
| `gdd/blueprint.md` | `assets/blueprint.md` | Levels, limits, checks, colors, zones and context, with its diagram, what an emergency may skip and how a change closes. Each level is a model by the name the agents' environment launches it with, never a description of how much it reasons. |
| `.gdd/gdd` | `references/layer.md` | The layer, set up before any step is logged. |
| `gdd/check.sh`, `gdd/limits.sh` | `assets/check.sh`, `assets/limits.sh` | A runner that times each check, unless the person says no: the constitution's commands go through it, `seal` apart. And the counter of what `limits` caps. |
| `gdd/gates/<gate>.feature` | `assets/gate.feature` | One per project gate. Use `assets/gates.feature` for a zone's own rules. |
| `gdd/backlog.md` | `assets/backlog.md` | Its first item, `standing-checks`, sets up what the blueprint's `checks` name, or with no code yet what `references/new-project.md` says, and CI where there is none. `gdd-checks` does it, because the agent that builds doesn't set up what judges it, and until CI runs the blueprint's `guarantee` is `rule`. Add the rest with `gdd backlog add`, and see that `gdd backlog list` reads every item. |
| `gdd/debt.md` | `assets/debt.md` | Only in an existing project with known debt. |
| GDD block of `AGENTS.md` | `assets/agents-block.md` | Under 100 words. If there is a `CLAUDE.md`, add `@AGENTS.md` to it. |

Three rules for the constitution of an alarm clock:

- A disabled alarm never rings.
- Snoozing one alarm never changes another.
- A weekday alarm stays silent on weekends.

Then the protections. Write a `CODEOWNERS` file that asks for the person's review on what the team always approves (`gdd/blueprint.md`, `gdd/gates/`, `gdd/check.sh`, `gdd/limits.sh`, `gdd/exam.md`, `gdd/constitution.md` and `gdd/debt.md`), on the agents' own files (`AGENTS.md`, `CLAUDE.md`, skills and settings) and on the zones they mark red. Add `features/` if the team wants every change to what has to pass to reach a person. Leave `gdd/log/`, `gdd/briefs/` and `gdd/backlog.md` out: agents write there on every change. Give the person the rest to apply on the platform, because it takes their credentials: a protected main branch that requires the checks to pass, the layer's too, a rule that keeps `gdd-seal/*` out of the builder's reach, and an agent confined to its work, with no access to production, its own credentials, a restricted network and none of its environment's apps, plugins or connectors that act on the person's accounts.

## The probe

A cage counts once you've seen it close. When the protections are in place, try four things from each environment in `levels`, in a role launched the way its `launch` says, and watch each one fail. Try each with something harmless you can remove, such as an empty commit or a ref named `gdd-seal/probe`, and if a try succeeds, undo it and tell the person. If you can't work from that environment, ask the person to run them there.

1. push to the main branch;
2. write to a `gdd-seal/` ref;
3. read the hidden examples, if the project has them (when a team adds them later, this try runs again);
4. find a credential of production or of the person within reach (an environment variable, a logged-in command line, a connector). Finding it is enough: never use it.

Each try goes in the `prepare` step's `--probe`: its command, exit code and last lines. If all of them fail in every one, the blueprint's `guarantee` is `platform`. If any succeeds, it's `rule`: say so to the person, write it in the blueprint and leave hidden examples out of red, because a builder that can read them isn't being tested by them.

## The baseline and the limits

Before the first change, write the baseline in the blueprint with the person, from the project's own history: how long a change takes from request to release, how often it stops someone and what slips through. A new project has no history: say so there, and take the baseline from its first changes. Then ask what figure would make the team drop a gate, and write it under it.

Set each color's caps from the wait they accept. Start with the template, then adjust from the logs. Above `lines_per_change`, tell the reviewer and note the size in the summary. Keep going.

## Approval

The person approves the first blueprint, and the approval stays written: a commit or a pull request of theirs, or their yes in this conversation where there's no platform. They approve the files as they are, so write every change into them before you ask, and show them the files, not a summary: whatever you change after their yes goes back to them. Ask with the message in `assets/approval.md`: copy it, fill it in and send nothing else, under 200 words, because what you decided is in the files. Recording known debt loosens a check, so it's part of that same approval. Tell the person what the trial of the standing checks leaves under `LATER:` and put it in the backlog, without waiting: it holds no change back, and what they approved stays as it is unless they change it. An agent never widens what has to pass, not even when the person leaves it to you.

## Gotchas

- Record existing failures in the baseline so they stay visible and later changes answer only for new failures.
- Some checks can't run inside the agent's box. Say which checks run outside it, and who runs them.
- Confine the agent's environment as well as the project, including plugins and credentials that can act on the person's accounts.

When you close, run `gdd step blueprint prepare --result <proposed or approved>` with a `--file` for each file, and `--probe` and `--levels` once tried.
