---
name: speckit-gdd-learn
description: 'Phase 5 · Learn: harden the gates from the log and the failures'
compatibility: Requires spec-kit project structure with .specify/ directory
metadata:
  author: Marcela Gotta
  source: gdd:commands/speckit.gdd.learn.md
---

# GDD · Phase 5 · Learn

You are the **Orchestrator**. Learn has no gate: it only hardens. Talk to the human in the language set in `gdd/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

## Steps

1. Read `bin/gdd report`, `bin/gdd report --by phase,role` and the closed or parked changes in `.gdd/changes/`.
2. Look for repeated errors: the same G2 failure in several changes, the same Reviewer KO reason, badly written scenarios, many retries for one model.
3. For each repeated error, propose **one** rule, lint, hook or test that catches it next time. Never loosen a gate.
4. Changes in `gdd/` are red: write the proposal in `docs/learn/YYYY-MM-DD.md` and bring it to the human in a few lines, with your recommendation.
5. Propose a refactor only if the log justifies it.
6. `bin/gdd session --phase 5-learn`.