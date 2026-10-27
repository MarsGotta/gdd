# GDD · Gate-Driven Development (v0.11, agent edition)

The working method of this repository. Work moves forward by passing gates. How many gates, and how strict, is decided by risk (Gate Level), not by the size of the work.

## Principles

1. Machines decide what can be checked; people decide what cannot.
2. Every rule lives in one place: `gdd/project.toml` (project basics), `gdd/zones.toml` (zone map), `gdd/rules.toml` (rules table), `gdd/models.toml` (agent and model per role) and `gdd/commands.toml` (gate commands). `bin/gdd`, the workflow and CI read those files.
3. Every loop has a limit. Nothing iterates forever.
4. Minimum context: each agent gets only what its task needs.
5. The human does not read code. They are the stakeholder: they approve red zones, scenarios and, sometimes, the demo, grouped in as few sittings as possible.
6. Every repeated error becomes a rule, lint, hook or test.
7. **Done is not good.** A change that passes every gate can still be unfinished: AI makes work look finished fast. The human is the editor at G4, and the project writes its quality bar in `docs/standards.md`, which the Builder and the Reviewer read.
8. Measure outcomes, not output: DORA's delivery metrics and GDD's own signals (polish rate, parks, Reviewer findings). Tokens measure cost, not productivity; lines of code are never a goal.

## Vocabulary

- **Change**: a small piece of value that goes through the gates as one unit, on its own branch `change/<id>`. It may cover one scenario, several or none (a refactor or a copy change adds no scenarios).
- **Scenario**: a Gherkin sentence (`Given… When… Then…`) in a `.feature` file that says what must happen. Scenarios are the contract the human approves in G1.
- **Project Brief**: `docs/brief.md`, the general idea of the project written in Setup and reused in Discover.

## Phases and gates (phase N ends in GN)

- **0 Setup → G0 Setup Gate.** Always first, once per repository. A few basic questions: what the project is, platform (mobile, desktop, web…), technology (the Orchestrator recommends one if the human has not decided) and what would be serious if it failed. Out of that come the Project Brief (with the sensitive areas in words), the technology ADR, the minimum skeleton and the harness with its self-test (4/4). **The machines open G0**: Brief written and harness 4/4. There is no zone map yet: only the harness itself is red, and unmapped code is yellow.
- **1 Discover → G1 Scenario Gate.** Starts from the Brief and goes deeper: research, options, spikes with fixed time and cost, MVP, kill rule and the next changes. At the end, with architecture and scenarios in hand, the Orchestrator maps the sensitive areas to paths (red zones). There is a G1 when a change creates or modifies a `.feature` (regression does not count). **One sitting** per change: the human approves the scenarios and, when there are any, the red zones (pending, changed or new) and the red plan. Until the red zones are approved only spikes can be built; when they change, they need approval again. Hidden examples reuse the sentences of the visible scenarios with new data: G1 refuses any other phrasing, because test mode never sees them and could not write their steps. If a Builder reports that it read them, the Orchestrator writes fresh hidden examples before G4 and runs them; the human decides at G4 with that result. Scenarios state everything the behavior depends on (the rules table, the zone map) in the Background, and any change to protected configuration happens before the step that starts the build: written after, Path Guard rightly blames the Builder and no Builder round can fix it.
- **2 Build → G2 Loop Gates.** Linter, types, tests, Path Guard and architecture rules on every round. The Builder has 3 retries; then it goes up to the Orchestrator (1 replan); then the change is parked and goes to the Decision Inbox. If the replan finds the fault in the steps or tests written in test mode, it sends the change back to test mode once before the build round. **Scope:** a path outside the package of the same or lower color is accepted and listed in the inbox; a higher color fails G2, and only the Orchestrator widens the scope. A relaunch whose code already passes G2 skips the Builder.
- **3 Integrate → G3 Merge Gate.** The automatic chain: all Gherkin scenarios in strict mode (in parallel, hidden examples included), static security analysis, new dependency checks, new-code analysis, mutation testing on the diff only in yellow and red. Then the Reviewer: OK or KO, a single fix. If it passes, it is merged and deployed **to pre only**. The merge commit carries the change's record (color, human approvals, Reviewer findings, decisions taken without the human, tokens); the working files stay in `.gdd/changes/<id>/`, which is not versioned. Every gate runs Path Guard before its commands and again after.
- **4 Release → G4 Release Gate.** Tests and an automatic demo in pre (video or screenshots of the scenarios; for a library or a tool without a screen, the readable run of its scenarios). The human answers OK, KO or **polish** (one more Builder round with their notes, then the gates and the demo again; it replaces a KO, it is not an extra stop) only if there was a G1 or the change is red; otherwise promotion is automatic when pre is green. **The same artifact** is promoted to production, without rebuilding. If errors rise, it rolls back to the previous version automatically. A promotion that fails leaves production untouched and the change waiting at G4: the same OK retries it. A change caught in the middle of a promotion parks with a message for the human (look at production first), so nothing is deployed twice.
- **5 Learn.** No gate. It only hardens: rules, lints, hooks, tests, lines of `docs/standards.md` (from repeated polish notes and Reviewer improvements), and removing unused code. Periodic refactoring.

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

Every agent or gate run appends one JSON line to `.gdd/log.jsonl`. `gdd` (one line per agent call, read from the agent's own JSON output) and `bin/gdd` do it on their own; the conversation with the human is added with `bin/gdd session --phase <phase>`.

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

## How it runs in this repository

- **`gdd`** (the Rust binary, installed from `main` in `.gdd/bin/gdd-rust`) runs the recipe: `bin/gdd launch <id>` and `bin/gdd resume <id> --g4 ok|ko|polish`. Build with 3 retries and 1 replan, G2, G3 with one fix, Reviewer with one fix, Merge Gate, pre, G4 pause and promotion. Every agent call uses the agent and model from `gdd/models.toml` for the change color, started as its agent sheet says.
- **Stack sheets** built into `gdd` (Rust, TypeScript, Python; `gdd stacks`): a project names one or more in `stack` of `gdd/project.toml`; `gdd stacks compose` shows the gate commands and protected paths they give together, and every agent gets the instructions of each stack (`gdd-stack-<name>`).
- **GDD commands** in every agent's folder, written by `gdd agents install` and checked by `gdd agents check`: with the human `gdd-status`, `gdd-setup`, `gdd-discover`, `gdd-change`, `gdd-learn`, `gdd-hotfix`, `gdd-report` (`/gdd-status` in Claude Code and OpenCode, `$gdd-status` in Codex); headless, started by the recipe: `gdd-build-test`, `gdd-build`, `gdd-replan`, `gdd-review`.
- **`bin/gdd`** runs `.gdd/bin/gdd-rust` for every command (gates, Path Guard, Decision Inbox, report, `selftest`, `ci`, `session`). `bin/gdd help`.
- **Agents** are started by `gdd` itself (since agent-launch): each agent's own program from its sheet, with the arguments of the role (no MCP servers; the Reviewer only reads; Builders cannot open gates or merge), an environment without API keys, and the tokens logged. Around every call `gdd` checks that no agent changed GDD state and that the Reviewer changed no file; either parks the change.
- **CI** (`.github/workflows/gdd.yml`): builds `gdd` from `main` and runs `bin/gdd ci` with it on the change, so the judge is the version before the change. G2 and G3 block there even if an agent skips a step.
- Spec Kit and the Python kit are no longer used here (removed after agent-sheets and kit-retire); the pilot repository still runs the kit until `gdd init`.
- **Local state** in `.gdd/` (not committed): log, changes, messages, self-test, G0 and the red zone approval.
