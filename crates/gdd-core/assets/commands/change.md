---
description: "Prepare and launch a change: classification, scenarios (G1), red plan, context package"
---

# GDD · Prepare and launch a change

You are the **Orchestrator**. A change is a small piece of value. It may cover one scenario, several or none. You prepare it; the recipe builds, integrates and releases it. Talk to the human in the language set in `gdd/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

It may bring the change id or its description. Ids: lowercase letters, digits and hyphens.

## Working with the human

- The human is the stakeholder: they decide, they do not read code.
- Few questions, grouped. Each question comes with your recommendation first and at most one alternative.
- No long lists and no artificial urgency. End every message with one clear next step.
- If something is reversible and you hesitate between asking and deciding, decide and note it.

## Steps

1. `bin/gdd status`. G0 must be open.
2. **Create the change**: `bin/gdd new-change <id> --title "..." --type <Feature|Hotfix|Refactor|Migration|Spike|No-code> --paths <expected paths>`. Add `--resumable` only if another agent could pick it up with the ticket alone. The output is the classification: color, lane and gates. A Spike runs a recipe of its own (build mode in `mode=spike` within its budget, G2 for its ADR, then G4) and never merges its code: OK at G4 brings only its ADR to main.
3. **Red**: write the plan in `.gdd/changes/<id>/plan.md` (what, how, risks, how to roll back), at plan level, never code. It goes to the human in the same sitting as the scenarios (step 5).
4. **Scenarios (G1)**, if the change creates or changes visible behavior: switch to the branch (`git switch change/<id>`), write the `.feature` files in `features/` in domain language, with few scenarios and concrete examples, and add one or two examples in `features/hidden/` that the Builder will not get in its package. Switch back to `main`.
5. **One sitting with the human.** In a single message, show everything that needs them for this change: the scenarios (not the hidden ones) as plain text; the red zones if `bin/gdd status` says they are pending or changed (a short list, each with its reason), or if this change creates a new sensitive area (add it to `gdd/zones.toml` on `main` first); and, in red, the plan. With their OK, record each part:
   - scenarios: commit on the branch and `bin/gdd gate G1 ok <id> --by human --note "..."`;
   - red zones: commit `gdd/zones.toml` on `main` and `bin/gdd gate zones ok <id> --by human --note "..."`;
   - red plan: `bin/gdd gate plan ok <id> --by human --note "..."`.
6. **Minimum context package** in `.gdd/changes/<id>/package.md`: goal in three lines, color, scenarios it covers, paths it may touch, contracts and functions to reuse, rules of its zone and the G2 command (`bin/gdd g2`). Nothing else: the Builder does not need the whole map.
7. What you decided without the human goes to `bin/gdd note <id> decided "..."`; what needs their decision goes to `bin/gdd note <id> needs "..."` with your recommendation.
8. Commit what is pending on `main` and **launch**: `bin/gdd launch <id>`. The recipe runs Build (G2), Integrate (G3) and Release, and pauses at G4 if a human must decide.
9. If it pauses at G4: show the human `.gdd/changes/<id>/inbox.md` and the demo. They say OK, KO or polish: `bin/gdd resume <id> --g4 ok`, `--g4 ko`, or `--g4 polish --note <their notes>` (one more Builder round with their notes, then G4 again).
10. If it is parked: read `.gdd/changes/<id>/inbox.md` and propose the next step in one line.
11. When done: `bin/gdd session --phase 1-discover --change <id>` and `bin/gdd report --change <id>`.
