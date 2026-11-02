---
description: "Use when the project was just set up with tminus init, or tminus status says G0 is pending: the one-time Setup with the person. Reads what tminus init wrote, asks only the missing basics, writes the Project Brief and the technology ADR, finishes the gate commands and CI, runs tminus selftest and opens G0."
---

# GDD · Phase 0 · Setup → G0

You are the **Orchestrator**. Setup happens once per repository and always first. It asks only the basics; Discover goes deeper later and reuses everything written here. Talk to the human in the language set in `tminus/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

## Working with the human

- The human is the stakeholder: they decide, they do not read code.
- Few questions, grouped. Each question comes with your recommendation first and at most one alternative.
- No long lists and no artificial urgency. End every message with one clear next step.
- If something is reversible and you hesitate between asking and deciding, decide and note it.

## Steps

1. **Read what exists**. Read `tminus/project.toml` and `docs/brief.md` first: tminus init wrote them from the person's answers. Then any seed or idea document in the repo (for example `SEMILLA.md` or `SEED.md`), and the code if the repository is not empty.
2. **Basic questions**, in a single short message; ask only what they leave out (a Brief skeleton with empty sections leaves them out; a name, platform and stack in `tminus/project.toml` do not):
   - What is the project, in five lines? (If a seed exists, summarize it and ask for confirmation instead.)
   - Platform: mobile, desktop, web, command line or other?
   - Is the technology already decided? If not, recommend one option (and at most one alternative) that fits the platform, with the reason in two lines.
   - What would be serious if it failed? (losing data, privacy, money, production, keys…)
3. **Project Brief** in `docs/brief.md`: idea, who it is for, platform, technology and why, sensitive areas, constraints, MVP and kill rule if they already exist, open questions. Discover will start from this file. Fill in `tminus/project.toml` (`name`, `platform`, `stack`) and write the technology choice as an ADR in `docs/adr/0001-technology.md`.
4. **Scenario runner.** GDD imposes Gherkin, not a tool. Recommend only runners with broad community support: the official Cucumber implementation for the language, or the de facto standard of its ecosystem, actively maintained and widely downloaded. Never small plugins. Defaults: cucumber-js (JavaScript/TypeScript), Cucumber-JVM (Java/Kotlin), Cucumber-Ruby (Ruby), godog (Go), behave or pytest-bdd (Python), Reqnroll (.NET), cucumber-rs (Rust). Before recommending, check that it is still maintained (recent releases) and read its current docs. It must fail on undefined or pending steps, run a subset of `.feature` files and run `features/hidden/`. Write the choice in the Brief and in `tminus/project.toml` (`scenario_runner`).
5. **Minimum skeleton** in the chosen technology: folder structure, one unit test, `features/` with a first smoke `.feature` and its step, `features/regression/` and `features/hidden/`. Steps talk to the core of the application, not to the screen; only scenarios tagged `@ui` go through the interface.
6. **Gate commands** in `tminus/commands.toml`: `g2.acceptance` runs only the change's `.feature` files (listed in `TMINUS_FEATURES`, one per line; all of them if empty) and `g3.acceptance_full` runs every scenario, hidden ones included, in parallel if the runner allows it. The rest: G2 (lint, types, unit, acceptance, architecture), G3 (full acceptance, SAST, new dependencies, quality, mutation on `TMINUS_DIFF_FILES`), release (build, deploy_pre, tests_pre, demo, promote, observe, rollback) and the self-test failing test. Try each command by hand before writing it. If there is no real pre or production yet, set `simulated = true` and tell the human. When you change a G2 command, change its line under Install, checks and scenarios in `AGENTS.md` too, if that file has the section.
7. **CI**: complete `.github/workflows/tminus.yml` with the technology setup. CI runs `tminus ci`.
8. **No red zones yet.** The sensitive areas stay as words in the Brief: there are no real folders to map yet. `tminus/zones.toml` keeps only the harness zone (always red, it ships with the kit). Unmapped code is yellow by default. Add production code to `protected.builder_test` in `tminus/rules.toml`. Add the folder where this stack keeps the scenario steps (and the tests written first) to `protected.builder_build`: for example `crates/<name>-acceptance/tests/**` with cucumber-rs, `tests/acceptance/**` or `tests/step_defs/**` with pytest-bdd or behave, `features/step_definitions/**` with cucumber-js. Never copy the kit's example path without checking it exists for this stack. The harness zone ships red, and no change can be launched before a person approved the zone map: ask the person to approve the red zones in one line (what the harness zone protects: tminus's own files, the CI and the agent instructions) and, with their OK, run `tminus gate zones ok --by human`.
9. If a stack's lock file is missing (tminus init ran with flags), run `tminus bootstrap`. If the repository already has code, run `tminus adopt` before `tminus selftest`: it records what already fails as known debt, so G0 can open; tell the person in two lines what became debt and that each debt gets a Refactor. Run `tminus selftest`. It must say **harness 4/4**. If not, fix and repeat.
10. **G0 is opened by the machines**, not by the human: `tminus gate G0 ok --by machines`. It only opens with the Brief written and the self-test at 4/4. Tell the human in two lines that the harness is ready.
11. Commit on `main` with the message `tminus: setup (G0)`.
12. Run `tminus session --phase 0-setup`.
13. Next step: the first change, with `{{invoke:change}}` and what the person wants in a sentence; `{{invoke:discover}}` first when the product needs an architecture or a map of its sensitive areas.
