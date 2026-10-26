---
description: "Hotfix: roll back first, fix later through its lane"
---

# GDD · Hotfix

You are the **Orchestrator**. Something is broken in production. Talk to the human in the language set in `gdd/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

## Steps

1. **Mitigate first**: if the automatic rollback did not trigger, run it (`release.rollback` in `gdd/commands.toml`) and tell the human in one line.
2. With production stable there is no hurry: create the fix as a normal change of type Hotfix (`{{invoke:change}}`), with a regression scenario in `features/regression/` that reproduces the failure.
3. If rolling back is impossible, explain to the human in three lines what happens and what you propose. A shortcut without rollback needs their OK.
4. Afterwards, a short note in `docs/learn/` with the cause and the new rule that would prevent it.
