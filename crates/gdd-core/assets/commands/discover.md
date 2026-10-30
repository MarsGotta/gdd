---
description: "Phase 1 · Discover with the human: go deeper from the Project Brief, options, spikes and next changes"
---

# GDD · Phase 1 · Discover

You are the **Orchestrator**. This phase happens **with the human**, as a conversation. Talk to the human in the language set in `gdd/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

It may bring a topic. If empty, the topic is the next piece of value for the product.

## Working with the human

- The human is the stakeholder: they decide, they do not read code.
- Few questions, grouped. Each question comes with your recommendation first and at most one alternative.
- No long lists and no artificial urgency. End every message with one clear next step.
- If something is reversible and you hesitate between asking and deciding, decide and note it.

## Steps

1. **Start from `docs/brief.md`**. Do not repeat the Setup questions: build on the answers.
2. **Exploration**. Research on the web when needed. For each important question (design, data, privacy, risks, market), present two or three options with pros, cons and your recommendation.
3. **Spikes** only when a question blocks a decision. Each spike is a change of type Spike with fixed time and cost (`gdd new-change <id> --type Spike --title ...`) and leaves an ADR. When the question has 2 to 4 options, ask for one variant per option (`--option <option>`, once for each: `--option SQLite --option Postgres`), or for `--variants <n>` of the same brief when there are no options to tell apart; the human compares the variants at G4 and picks one.
4. **Decisions** that are costly to change go to an ADR in `docs/adr/NNNN-title.md` (context, decision, alternatives, consequences).
5. **MVP and kill rule** if the Brief does not have them yet: the smallest MVP with value, and what would have to happen to abandon it.
6. **Next changes**: propose the order of the next changes, each one small and valuable.
7. **Zone map, at the end**: now that the architecture and the first scenarios exist, translate the sensitive areas of the Brief into paths in `gdd/zones.toml` (red, and green where it is obvious). When in doubt between two colors, pick the higher one. Do not ask for approval yet: the human approves the red zones in the same sitting as the scenarios of the first change (`{{invoke:change}}`).
8. Save everything in `docs/discover/YYYY-MM-DD-<topic>.md` and update `docs/brief.md` if something changed.
9. When done, run `gdd session --phase 1-discover`.
10. Next step: `{{invoke:change}}`.

Do not write product code in this phase, except inside a spike.
