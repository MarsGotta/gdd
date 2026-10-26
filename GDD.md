# GDD · Gate-Driven Development (v0.11, agent edition)

The working method of this repository. Work moves forward by passing gates. How many gates, and how strict, is decided by risk (Gate Level), not by the size of the work.

## Principles

1. Machines decide what can be checked; people decide what cannot.
2. Every rule lives in one place: `gdd/project.toml` (project basics), `gdd/zones.toml` (zone map), `gdd/rules.toml` (rules table), `gdd/models.toml` (agent and model per role) and `gdd/commands.toml` (gate commands). `bin/gdd`, the workflow and CI read those files.
3. Every loop has a limit. Nothing iterates forever.
4. Minimum context: each agent gets only what its task needs.
5. The human does not read code. They are the stakeholder: they approve red zones, scenarios and, sometimes, the demo, grouped in as few sittings as possible.
6. Every repeated error becomes a rule, lint, hook or test.

## Vocabulary

- **Change**: a small piece of value that goes through the gates as one unit, on its own branch `change/<id>`. It may cover one scenario, several or none (a refactor or a copy change adds no scenarios).
- **Scenario**: a Gherkin sentence (`Given… When… Then…`) in a `.feature` file that says what must happen. Scenarios are the contract the human approves in G1.
- **Project Brief**: `docs/brief.md`, the general idea of the project written in Setup and reused in Discover.

## Phases and gates (phase N ends in GN)

- **0 Setup → G0 Setup Gate.** Always first, once per repository. A few basic questions: what the project is, platform (mobile, desktop, web…), technology (the Orchestrator recommends one if the human has not decided) and what would be serious if it failed. Out of that come the Project Brief (with the sensitive areas in words), the technology ADR, the minimum skeleton and the harness with its self-test (4/4). **The machines open G0**: Brief written and harness 4/4. There is no zone map yet: only the harness itself is red, and unmapped code is yellow.
- **1 Discover → G1 Scenario Gate.** Starts from the Brief and goes deeper: research, options, spikes with fixed time and cost, MVP, kill rule and the next changes. At the end, with architecture and scenarios in hand, the Orchestrator maps the sensitive areas to paths (red zones). There is a G1 when a change creates or modifies a `.feature` (regression does not count). **One sitting** per change: the human approves the scenarios and, when there are any, the red zones (pending, changed or new) and the red plan. Until the red zones are approved only spikes can be built; when they change, they need approval again. Hidden examples reuse the sentences of the visible scenarios with new data: G1 refuses any other phrasing, because test mode never sees them and could not write their steps.
- **2 Build → G2 Loop Gates.** Linter, types, tests, Path Guard and architecture rules on every round. The Builder has 3 retries; then it goes up to the Orchestrator (1 replan); then the change is parked and goes to the Decision Inbox. If the replan finds the fault in the steps or tests written in test mode, it sends the change back to test mode once before the build round. **Scope:** a path outside the package of the same or lower color is accepted and listed in the inbox; a higher color fails G2, and only the Orchestrator widens the scope. A relaunch whose code already passes G2 skips the Builder.
- **3 Integrate → G3 Merge Gate.** The automatic chain: all Gherkin scenarios in strict mode (in parallel, hidden examples included), static security analysis, new dependency checks, new-code analysis, mutation testing on the diff only in yellow and red. Then the Reviewer: OK or KO, a single fix. If it passes, it is merged and deployed **to pre only**.
- **4 Release → G4 Release Gate.** Tests and an automatic demo in pre (video or screenshots of the scenarios; for a library or a tool without a screen, the readable run of its scenarios). The human says OK or KO only if there was a G1 or the change is red; otherwise promotion is automatic when pre is green. **The same artifact** is promoted to production, without rebuilding. If errors rise, it rolls back to the previous version automatically.
- **5 Learn.** No gate. It only hardens: rules, lints, hooks, tests. Periodic refactoring.

## Scenarios: the method, not the tool

GDD requires **Gherkin** (`.feature` files in `features/`), not a specific runner. Each project picks the runner that fits its technology in the Setup, choosing only among runners with broad community support (the official Cucumber implementation or the de facto standard of the ecosystem). Whatever the runner, it must:

- **fail on undefined or pending steps** (strict mode);
- **run only some `.feature` files**: G2 runs the scenarios of the change (`GDD_FEATURES`), G3 runs all of them;
- **run the hidden examples** in `features/hidden/`.

Speed comes from where the steps go, not from the runner: by default, steps talk to the core of the application (its domain, through its ports), not to the screen. Only a few scenarios go through the user interface, tagged `@ui`; the real interface is also checked in the pre demo.

## Gate Level (traffic light)

`bin/gdd classify` computes the color from the touched paths and `gdd/zones.toml`. The highest color wins. When in doubt, the higher one. Only three colors. By default, code without tests is yellow.

- **Green:** cheap or local Builder. No mutation testing. Fast Lane possible.
- **Yellow:** characterization tests before touching. Mutation testing on the diff. Reviewer always.
- **Red:** the human approves the plan (not the code). A more capable Builder. The Reviewer also checks security. G4 always. Feature flag and gradual rollout. Never Fast Lane.

Color beats work type and lane.

## Lanes

- **Fast Lane** (G2 + G3, 0 human stops) when all three hold: not red, no `.feature` created or changed, and it passes the resumption test (another agent could pick it up with the ticket alone). The Orchestrator judges it and it is logged.
- **Full Lane:** everything else, as set in `gdd/rules.toml`.

Work types: Feature, Hotfix (automatic rollback first; the fix later, through its lane), Refactor, Migration (expand/contract; the old path is removed after N days without use), Spike (fixed time and cost; produces an ADR in `docs/adr/`) and No-code.

## Roles

- **Orchestrator** (strong model). Holds the map: architecture, ADRs, zones, contracts, scenarios, state and log. Classifies, splits work into changes, prepares minimum context packages, does 1 replan per stuck task, groups open items for the human and hardens gates in Learn. Does not write code, does not resolve merge conflicts and never loosens gates.
- **Builder** (cheap or local in green; more capable in red). Two modes, always in separate sessions:
  - test mode: step definitions for the Gherkin scenarios, against the domain language, with real assertions;
  - build mode: domain, wiring and unit tests until green.
  Never edits approved tests or protected files, never silences warnings. Works in an isolated environment, without production credentials.
- **Reviewer** (a different model family than the Builder). Sees the whole change and the gate results. OK or KO with reasons. Checks that every step does what its scenario says and that there are no special cases. One fix. Opens no new rounds.
- **Machines.** They only say yes or no, and why.
- **Human.** Setup answers, Discover, one sitting per change (scenarios, red zones and red plan), G4 when it applies and one message per change.

## Human stops

G0: 0 (machines) · Fast Lane: 0 · Full Lane: 0 to 2 · red: 2 (plan + scenarios + zones in one sitting, then G4) · hotfix: 0 or 1 · at most 1 message per change. Mnemonic: the machines open 0; the human opens 1 and sometimes 4; the machines guard 2 and 3.

## Decision Inbox

A single message per change: what was decided without the human, what needs them (with a recommendation and at most one alternative) and what was parked. It lives in `.gdd/changes/<id>/inbox.md`.

## Log (mandatory from the first step)

Every agent or gate run appends one JSON line to `.gdd/log.jsonl`. The agent wrapper (`.gdd/bin/gdd-*`) and `bin/gdd` do it on their own; the conversation with the human is added with `bin/gdd session --phase <phase>`.

```json
{"ts":"...","change":"...","phase":"...","gate":"...","role":"orchestrator|builder-test|builder-build|reviewer|machine|human","agent":"...","model":"...","tokens_in":0,"tokens_cache":0,"tokens_out":0,"cost_usd":0,"duration_s":0,"result":"ok|ko|error","retry":0}
```

Where tokens come from:
- Claude Code: JSON output of `claude -p` (usage and `total_cost_usd`, a client-side estimate). Interactive session: its local transcripts.
- Codex: `turn.completed` events from `codex exec --json`. No cost: computed from `[prices]` in `gdd/models.toml` if filled in.
- OpenCode: `step_finish` events from `opencode run --format json`, with tokens and cost.
- With a subscription, cost is notional: tokens are what compares.

Report: `bin/gdd report --by change,model,role` (any combination of `change`, `phase`, `model`, `role`, `agent`).

## Language

The tool, its files and its commands are in English. Agents talk to the human in the language set in `gdd/project.toml` (`language`).

## How it runs: Spec Kit and the GDD kit

- **Spec Kit** with three integrations installed: `claude`, `codex` and `opencode`.
- **`gdd` extension** (source in `gdd-kit/`, installed for every integration with `gdd-kit/install.sh`: Spec Kit 1.2 registers extension commands only for the active integration). Commands:
  - with the human: `/speckit-gdd-status`, `/speckit-gdd-setup`, `/speckit-gdd-discover`, `/speckit-gdd-change`, `/speckit-gdd-learn`, `/speckit-gdd-hotfix`, `/speckit-gdd-report`;
  - headless (launched by the workflow): `build-test`, `build`, `replan`, `review`.
- **`gdd-change` workflow** (`gdd-kit/workflows/gdd-change/`): Build with 3 retries and 1 replan, G2, G3 with one fix, Reviewer with one fix, Merge Gate, pre, G4 as a `gate` step (pauses and resumes) and promotion. Every agent step uses the agent and model from `gdd/models.toml` for the change color.
- **`bin/gdd`**: the gate tool (classify, G2, G3, Path Guard, verdict, gates, merge, release, Decision Inbox, report). `bin/gdd help`.
- **Agent wrapper** (`.gdd/bin/gdd-claude`, `gdd-codex`, `gdd-opencode`): Spec Kit launches every agent through it. It removes MCP servers, applies permissions (the Reviewer only reads; Builders cannot open gates or merge) and logs tokens.
- **CI** (`.github/workflows/gdd.yml`): runs `bin/gdd ci`. G2 and G3 block there even if an agent skips a workflow step.
- **Kit test**: `gdd-kit/tests/e2e.sh` runs four cases with fake agents, spending no tokens. Run it whenever the kit changes.
- **Local state** in `.gdd/` (not committed): log, changes, messages, self-test, G0 and the red zone approval.
