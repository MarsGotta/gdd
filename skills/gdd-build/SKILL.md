---
name: gdd-build
description: Builds one change through its gates in a Gate-Driven Development (GDD) project, from its brief and its sealed checks. Loaded only by the agent the GDD orchestrator launches to do the work. Not for writing or changing checks, scenarios or the blueprint.
license: MIT
metadata:
  version: "0.13.1"
  contract: gdd/1
---

# Building a change

Build this change. Its gates decide when it's done. Never orchestrate, write checks, approve or launch agents.

- Follow the brief (otherwise the request), stay within its paths and commit on your branch.
- If git can't add your worktree inside the project, clone it in your temporary folder with its git identity, start at `<start>` (later at the project's branch tip) and run `git fetch <clone> <branch>:<branch>` from the project before reporting.
- Leave sealed checks, `gdd/`, `features/` and agents' files untouched. Never seek hidden examples. Never weaken, skip, delete or retry a test or raise its timeout to pass.
- Save command output to files and read only their tails. Don't print unchanged files twice. Run every gate with the constitution's command. Report command, exit code and last five lines. Cite the scenario for failures.
- Demonstrate pre-existing failures on the base and leave them alone.
- Stop for a needed path outside scope, a check that looks wrong or contradicts the brief, or impossible work. Begin your last message with `CANNOT:`, why and what would unblock you. Never fake a solution or copy code around a path limit.

End with three lines (work, each gate's evidence, anything unchecked), then one filled block. Write `result: done` when finished, `result: stopped` after `CANNOT:`. One `decision:` line per remaining product choice.

```gdd-report
skill: gdd-build 0.13.1
result: <done or stopped>
caught: <what it caught that others missed, or none>
decision: <a product choice the brief or request left open, or none. Include numbers, order, limits, defaults and what an outside service can or cannot do>
```

Completed example

```gdd-report
skill: gdd-build 0.13.1
result: done
caught: none
decision: New sketches start with a 5 mm grid.
```
