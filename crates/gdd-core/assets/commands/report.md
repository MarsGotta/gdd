---
description: "Token report by change, phase, model and role"
---

# GDD · Report

Talk to the human in the language set in `gdd/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

1. Run `gdd session --phase <current phase>` to include this conversation.
2. Run `gdd report --by change,role,model` (or the grouping the human asks for: `change`, `phase`, `model`, `role`, `agent`, comma separated).
3. Summarize in a few lines: total, which role and which model spend the most, retries and human stops. If some agent shows cost 0, explain that it does not report price and that with a subscription the cost is notional: tokens are what compares.
