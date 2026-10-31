# Glossary

The words these pages use, in alphabetical order, each with a short definition and a link to
the page that explains it. Consult it when a page uses a word you do not know. When two pages
seem to disagree on a word, this page and the page it links to win.

## ADR

An Architecture Decision Record: a short file in `docs/adr/` with the context, the decision, the
alternatives and the consequences of a choice that is costly to change. Every spike ends in one.
See [ADR 0001](adr/0001-technology.md) for a real example.

## Agent sheet

The data gdd keeps about each coding agent it can start (Claude Code, Codex, OpenCode): its
program, where its commands go, how to launch it headless, the arguments of each role and how to
read its tokens. See [Keeping agents honest](explanation/safety.md#layer-3-what-an-agent-can-do-at-all).

## Board

The read-only terminal screen that shows every change by phase, with its color, state and gates.
It opens with [`gdd board`](reference/cli/board.md), or `gdd` alone, in a terminal.

## Build mode

The Builder's second mode: it writes the domain, the wiring and the unit tests until G2 is green,
and may not touch the steps written in test mode. See
[The life of a change](explanation/life-of-a-change.md#4-build-mode-and-g2).

## Builder

The agent role that writes the steps, the tests and the code, in two modes: test mode and build
mode. Its model follows the color: cheap or local in green, more capable in red. See
[Roles](reference/roles.md).

## Change

One small piece of value that goes through the gates as a unit, on its own branch `change/<id>`.
It may cover one scenario, several or none. Always a *change*, never a task or a ticket. See
[GDD in one page](explanation/method.md).

## Characterization test

A test that pins down what existing code does today, right or wrong, so that any change to that
behavior shows up. Test mode writes them before a yellow or red change touches existing code.
See [Skills of gdd's agents](reference/skills/agent-skills.md#gdd-build-test).

## Color

The risk level of a path or a change: green, yellow or red. A change takes the highest color among
the paths it touches; the color decides its gates. Also called its Gate Level. See
[How risk picks the gates](explanation/risk.md).

## Core

The part of GDD that guards against the agents and stays on whatever the configuration says: the
gates G1 to G4, the traffic light (the three [colors](#color)), Path Guard, hidden examples, a
Reviewer of another model family and mutation testing on red changes. See
[Design decisions](explanation/design-decisions.md#minimal-by-default-incremental-by-choice).

## Decision Inbox

The single message a change leaves for the human, in `.gdd/changes/<id>/inbox.md`: what needs
their decision, what was decided without them and what was parked. See
[Few stops for the human](explanation/human-stops.md#the-decision-inbox).

## Discover

Phase 1: research, options, spikes, the MVP and its kill rule, and the next changes, worked out
with the human. It ends by mapping the red zones. See [GDD in one page](explanation/method.md#phases-and-gates).

## Fast Lane

The lane with G2 and G3 and no human stop, open only to a change that is not red, creates or
changes no scenario and is resumable. See [How risk picks the gates](explanation/risk.md#the-lanes).

## Feature

The default work type: new or changed behavior, usually with new scenarios. See
[Work type](#work-type).

## Full Lane

The lane every change takes unless it qualifies for the Fast Lane, with the gates the rules table
gives its color. See [How risk picks the gates](explanation/risk.md#the-lanes).

## G0 Setup Gate

The gate that ends phase 0. The machines open it, once the Project Brief exists and the harness
self-test passes all four checks. No change starts before it. See [Gates](reference/gates.md).

## G1 Scenario Gate

The gate where the human approves a change's new or changed scenarios, in the same sitting as the
red zones and the red plan. Approving seals the scenario files with their hashes. See
[Write and approve scenarios](how-to/write-scenarios.md).

## G2 Loop Gate

The checks run after every Builder round: linter, types, tests, the change's scenarios,
architecture rules, Path Guard and the scope check. See
[The life of a change](explanation/life-of-a-change.md#4-build-mode-and-g2).

## G3 Merge Gate

The full automatic chain before the merge (every scenario including the hidden examples, security,
dependencies, secrets and, by color, mutation testing), then the Reviewer. See
[The life of a change](explanation/life-of-a-change.md#5-g3-the-merge-gate).

## G4 Release Gate

The gate between pre and production. The human answers OK, KO or polish when the change is red or
had a G1; otherwise the machines promote it once pre is green. See
[Answer the Release Gate](how-to/release-gate.md).

## Gate

A checkpoint a change must pass to move on. It answers yes or no, and says why. See
[GDD in one page](explanation/method.md#the-idea).

## Gate Level

Another name for a change's [color](#color).

## Gherkin

The plain-language format of the scenarios: a `Feature` with `Scenario`s whose steps start with
`Given`, `When` and `Then`. GDD requires Gherkin, not a tool; the Setup picks the scenario
runner for the stack. See [gdd-setup](reference/skills/setup.md).

## Harness

The files that run and protect the method: `gdd/`, the continuous integration (CI) workflow and
the agents' folders. It is a red zone and protected from every role. See
[Keeping agents honest](explanation/safety.md#layer-1-path-guard).

## Hidden example

An extra example in `features/hidden/` that reuses the sentences of the visible scenarios with
new data. The Builder's package leaves it out; G3 runs it. See
[Keeping agents honest](explanation/safety.md#layer-2-judges-the-builder-cannot-reach).

## Hotfix

The work type for something broken in production: roll back first, then fix it through its lane,
with a regression scenario. Its color still decides its gates. See [Work type](#work-type).

## Human

The person who uses GDD: the stakeholder who approves scenarios, red zones, red plans and
releases, and does not read code to do it. See
[Few stops for the human](explanation/human-stops.md).

## Kill rule

What would have to happen to abandon a minimum viable product (MVP), written in Discover before the work starts and in
numbers the reports can show. See [Measuring what matters](explanation/measuring.md#the-kill-rule).

## Known debt

A gate command that already failed when a project with code adopted GDD. It warns instead of
blocking, in the gates and in the self-test, until a Refactor turns it green, and the [ratchet](#ratchet) keeps it from growing. Kept
in `gdd/debt.toml`. See [Bring gdd to an existing project](how-to/adopt-existing-project.md).

## Lane

The route a change travels: the [Fast Lane](#fast-lane) or the [Full Lane](#full-lane). The lane
never lowers the color. See [How risk picks the gates](explanation/risk.md#the-lanes).

## Learn

Phase 5, with no gate: repeated problems become rules, lints, hooks, tests and lines of
`docs/standards.md`. `gdd learn` lists the repeats. See
[Few stops for the human](explanation/human-stops.md#learn-turns-repeated-notes-into-rules).

## Lock file

The file where a stack's package manager pins the exact versions of the dependencies, such as
`uv.lock` for Python. `gdd init` commits it after the first install, `gdd bootstrap` writes it
when it is missing, and G3 checks it. See [Stacks](reference/stacks.md).

## Merge queue

How changes reach main one at a time: under a merge lock, a change whose main moved takes the
latest main and passes G3 again before it merges. See
[Run changes in parallel](how-to/parallel-changes.md).

## Migration

The work type for a change made in two steps, expand then contract: the old path is removed after
a set number of days without use. See [Work type](#work-type).

## Model family

The provider family of a model, from `[families]` in `gdd/models.toml`. The Reviewer must be of
another family than the Builder, so they do not share blind spots. See
[Choose agents and models](how-to/agents-and-models.md).

## Mutation testing

A test of the tests: a tool makes small edits to the code, called mutants, such as flipping a
comparison, and the tests must fail for each one. A mutant that no test catches shows a test
that checks too little. gdd runs the `mutation*` commands in G2 and G3, always for red changes
and for other colors when a rule turns them on. See [Gates](reference/gates.md#gates-by-color).

## No-code

The work type for a change that adds no code. Like a Spike or an invisible Refactor, it gets no
changelog line unless one is set. See [Work type](#work-type).

## Orchestrator

The agent role that holds the map (architecture, zones, scenarios, state), splits work into
changes, writes their packages, replans a stuck change once and groups what needs the human. It
never writes code. See [Roles](reference/roles.md).

## Package

The minimum context the Orchestrator writes for a change, in `package.md`: the goal, the color,
the scenarios, the paths it may touch, what to reuse and the G2 command. See
[Design decisions](explanation/design-decisions.md#roles-with-minimum-context).

## Park

To stop a change with a message for the human in its Decision Inbox, when a limit is reached or a
check fails. A new `gdd launch` picks it up again. See
[Get a parked change going again](how-to/parked-change.md).

## Path Guard

The check that a role touched none of its protected paths and no scenario sealed at G1, since the
change's build started. It runs before and after every gate's commands. See
[Keeping agents honest](explanation/safety.md#layer-1-path-guard).

## Polish

The third answer at G4: one more Builder round with the human's notes, then the gates and the demo
again. It replaces a KO; it is not an extra stop. See
[Few stops for the human](explanation/human-stops.md#done-is-not-good).

## Pre

The environment a merged change is deployed to before production, where its tests in pre and its
demo run. See [The life of a change](explanation/life-of-a-change.md#7-merge-then-pre).

## Production

Where users get the change. Promotion takes the same artifact from pre, without rebuilding it,
and rolls back if errors rise. See
[The life of a change](explanation/life-of-a-change.md#9-promote-observe-close).

## Project Brief

`docs/brief.md`: what the project is, for whom, on which platform and technology, and what would
be serious if it failed, written at the Setup. See gdd's own [Brief](brief.md).

## Ratchet

A ceiling on known debt: `gdd adopt` counts what each failing command reports, a change that raises
the count fails the gate, and one that lowers it lowers the ceiling for good. See
[Bring gdd to an existing project](how-to/adopt-existing-project.md).

## Record

A change's facts, `.gdd/changes/<id>/change.json`, which only gdd writes: its decision, approvals,
reviews, notes, state and what it spent. Its merge commit carries a summary into git. See
[Keeping agents honest](explanation/safety.md#layer-4-a-record-only-gdd-writes).

## Red plan

The plan of a red change, at plan level, never code: what, how, risks and how to roll back. The
human approves it in the same sitting as the scenarios. See
[Few stops for the human](explanation/human-stops.md#one-sitting-per-change).

## Red zone

A path in the zone map where a mistake would be serious. The human approves the red zones, and
approves them again when they change. See [Map and approve red zones](how-to/red-zones.md).

## Refactor

The work type for changing how code is built without changing what it does. See
[Work type](#work-type).

## Regression scenario

A scenario under `features/regression/`, usually pinning a bug. Changing only these needs no G1.
See [How risk picks the gates](explanation/risk.md#the-lanes).

## Replan

The Orchestrator's one try at unsticking a change whose G2 is still red after the Builder's rounds.
It ends with `NEXT: build`, or `NEXT: test` to send the change back to test mode once. See
[The life of a change](explanation/life-of-a-change.md#4-build-mode-and-g2).

## Reviewer

The agent role, of another model family than the Builder, that reads the whole change and the gate
results and answers OK or KO with reasons. It only reads, and gets one fix. See
[Roles](reference/roles.md).

## Scenario

A Gherkin sentence (`Given... When... Then...`) in a `.feature` file that says what must happen.
The scenarios are the contract the human approves at G1. See
[Write and approve scenarios](how-to/write-scenarios.md).

## Scope

The paths a change plans to touch. A path outside the plan of the same or a lower color is
accepted and listed; one of a higher color fails G2. See
[How risk picks the gates](explanation/risk.md#the-scope-rule).

## Self-test

`gdd selftest`: four checks that the gates fail when they must. The machines open G0 only when all
four pass. See [Keeping agents honest](explanation/safety.md#the-self-test).

## Setup

Phase 0, once per project: a few basic questions, the Project Brief, the technology ADR, the
skeleton and the harness. See [GDD in one page](explanation/method.md#phases-and-gates).

## Sitting

The one moment before Build when the human approves everything a change needs from them: the
scenarios, the red zones and the red plan. G4 has a sitting of its own. See
[Few stops for the human](explanation/human-stops.md#one-sitting-per-change).

## Snapshot

The hashes of what a change touched at one moment. The build snapshot, taken after test mode,
makes build mode answer only for its own work. See
[Keeping agents honest](explanation/safety.md#layer-1-path-guard).

## Spike

A change that answers one question so the human can decide: built fast within a fixed budget,
ending in an ADR. Only the ADR reaches main; its code never does. See
[Explore with a spike](how-to/spikes.md).

## Stack sheet

The data gdd ships for a technology (Rust, TypeScript, Python): gate commands, scenario runner,
protected step paths and instructions for the agents. A project may combine several. See
[Stacks](reference/stacks.md).

## Standards

`docs/standards.md`: the project's quality bar, in words. The Builder and the Reviewer read it, and
Learn adds to it. See gdd's own [standards](standards.md).

## Test mode

The Builder's first mode: it writes the step definitions of the scenarios, with real assertions,
and characterization tests in yellow and red. It may not write production code. See
[The life of a change](explanation/life-of-a-change.md#3-test-mode).

## Variant

One of two to four versions of a spike, each built by its own Builder on its own branch, compared
on one page at G4. See [Explore with a spike](how-to/spikes.md).

## Work type

The kind of work a change is: Feature, Hotfix, Refactor, Migration, Spike or No-code. It never
changes the color; a spike keeps its color but skips the gates its code would need, since its code
never merges. See [How risk picks the gates](explanation/risk.md#color-beats-work-type-and-lane).

## Worktree

The separate git working folder where a change builds, `<repo>.worktrees/<id>` by default, so the
main checkout stays on main. See [The life of a change](explanation/life-of-a-change.md#1-launch).

## Zone map

`gdd/zones.toml`: the colors of the project's paths, with a default color for the rest (yellow).
See [How risk picks the gates](explanation/risk.md#the-zone-map).

## See also

- [GDD in one page](explanation/method.md): the terms in context.
- [The method's full rules](../GDD.md): the source of every definition.
