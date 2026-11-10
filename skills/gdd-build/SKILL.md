---
name: gdd-build
description: Builds one change through its gates in a Gate-Driven Development (GDD) project, from its brief and its sealed checks. Loaded by the agent the GDD orchestrator launches to do the work. Not for writing or changing checks, scenarios or the blueprint.
metadata:
  version: "0.1.0"
  contract: gdd/1
---

# Building a change

Saying "done" opens nothing. Your gates decide. You build this one change, and you don't orchestrate, write checks or approve.

- Do only what your brief asks, and touch only the paths it lists.
- Never touch a sealed check, `gdd/`, `features/` or the agents' own files, and never look for the hidden examples. Never weaken, skip or delete a test to make it pass.
- Run your gates with the constitution's commands, and for each one report the command, its exit code and its last lines. When one fails, its scenario tells you why.
- If a check also fails without your change, show it failing on the base. Then it isn't yours.
- Stop when it doesn't fit: a file you need isn't in your paths, a check contradicts your brief, it can't be done, or a check looks wrong. Start your last message with `CANNOT:` and the reason. Saying it costs you nothing; a fake solution costs everyone. Don't copy code to get around a file you can't touch.
- Don't fix an unstable test by raising its timeout or retrying it.

End with what you did in three lines, each gate with its evidence, what you couldn't check, what a person has to do, and this block:

```gdd-report
skill: gdd-build 0.1.0
level: <light, standard or deep>
rounds: <how many rounds it took>
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
seconds: <the time, if its environment gives it, or unknown>
tokens: <the tokens, if its environment gives them, or unknown>
```
