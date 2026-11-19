---
name: gdd-build
description: Builds one change through its gates in a Gate-Driven Development (GDD) project, from its brief and its sealed checks. Loaded only by the agent the GDD orchestrator launches to do the work. Not for writing or changing checks, scenarios or the blueprint.
license: MIT
metadata:
  version: "0.16.0"
  contract: gdd/1
---

# Building a change

Gates decide when you're done. Never orchestrate, write checks, approve or launch agents.

- Build from the brief (otherwise the request) within its paths. Commit on your branch.
- Use your own worktree inside the project, from `<start>`, later the project's branch tip. If git cannot add it, follow `../gdd/references/role-clone.md`. Never force a refused fetch: stop with `CANNOT:`.
- Never touch sealed checks, `gdd/`, `features/` or agents' files. Never seek hidden examples. Never weaken, skip, delete or retry a test or raise its timeout to pass.
- Save command output and read only the file tails. Don't print unchanged files twice. That fills your context. Run every gate with the constitution's command. Report command, exit code and last five lines. A failing gate's scenario says why.
- Prove pre-existing failures on the base. Leave them alone.
- Stop when you need a file outside your paths, a check looks wrong or contradicts the brief, or the work is impossible. Start your last message with `CANNOT:`, why and what would unblock you. Never fake a solution or copy code around a path limit. A fake solution wastes everyone's time.

End with three lines (work, each gate's evidence, anything unchecked), then one filled block. Write `result: done` when finished, `result: stopped` after `CANNOT:`. Give each product choice still in the change one `decision:` line.

```gdd-report
skill: gdd-build 0.16.0
result: <done or stopped>
caught: <what it caught that others missed, or none>
decision: <a product choice the brief or request left open, or none. Include numbers, order, limits, defaults and what an outside service can or cannot do>
```

Example

```gdd-report
skill: gdd-build 0.16.0
result: done
caught: none
decision: New sketches start with a 5 mm grid.
```
