---
description: "Use when the person asks what the agents cost or where the time went: tokens, cost and time by change, phase, role, model or agent. Adds this session with tminus session, runs tminus report and sums it up in a few lines."
---

# GDD · Report

Talk to the human in the language set in `tminus/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

1. Run `tminus session --phase <current phase>` to include this conversation.
2. Run `tminus report --by change,role,model` (or the grouping the human asks for: `change`, `phase`, `model`, `role`, `agent`, comma separated).
3. Summarize in a few lines: total, which role and which model spend the most, retries and human stops. A cost marked ~ is an estimate from the prices in tminus/models.toml; unknown means the agent reported no cost and its model has no price there (the report names those models). Say so, and that with a subscription the cost is notional: tokens are what compares.
