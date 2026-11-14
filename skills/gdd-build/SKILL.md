---
name: gdd-build
description: Builds one change through its gates in a Gate-Driven Development (GDD) project, from its brief and its sealed checks. Loaded only by the agent the GDD orchestrator launches to do the work. Not for writing or changing checks, scenarios or the blueprint.
license: MIT
metadata:
  version: "0.11.3"
  contract: gdd/1
---

# Building a change

Saying "done" opens nothing. Your gates decide. You build this one change: you don't orchestrate, write checks, approve or launch other agents.

- Do what your brief asks (the request, when there's no brief) as a change in the repository, not as an answer about it. Touch only its paths, and commit on your branch.
- Never touch a sealed check, `gdd/`, `features/` or the agents' own files, and never look for the hidden examples. Never weaken, skip or delete a test to make it pass, nor raise its timeout or retry it.
- Run your gates with the constitution's commands, their output to a file, and report each one's command, exit code and last five lines. When one fails, its scenario tells you why.
- A check that also fails without your change isn't yours: show it failing on the base.
- Stop when it doesn't fit: a file you need isn't in your paths, a check contradicts your brief or looks wrong, or it can't be done. Start your last message with `CANNOT:`, the reason and what would unblock you. Saying it costs nothing; a fake solution costs everyone. Don't copy code to get around a file you can't touch.

End with three lines on what you did, each gate's evidence, what you couldn't check, and this block, where `result` is `done` or `stopped`.

```gdd-report
skill: gdd-build 0.11.3
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
```
