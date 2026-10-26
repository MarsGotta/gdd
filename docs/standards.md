# Standards · gdd

What good looks like in this project, in words. The Builder and the Reviewer read this file; Learn adds to it when the same polish note or the same Reviewer finding comes back. Gates check what a machine can check; this file is for the rest.

Draft by the Orchestrator from the first five changes. The human edits it.

## For the person using gdd

- Every message says what happened and what to do next, in one or two lines. An error names the file, the line or the command involved.
- Output for machines is JSON on stdout; reasons and errors go to stderr. Never mix them.
- English in commands, files and messages. The agents talk to the human in the language set in `gdd/project.toml`.
- A refusal is never silent: if gdd will not do something (approve, merge, promote), it says why and who can.

## For the code

- Small, named pieces over clever ones. If a function needs a comment to be followed, split it.
- No `unwrap()` or `expect()` on anything that comes from the user, a file or a command.
- Git and shell commands only in their own modules, with fixed arguments; agent text never reaches a command.
- No new dependency without a reason in the plan. Fewer lines are better than more: delete what no scenario needs.
- Do not keep code «for later». A spike's code does not enter main as it is.

## For the scenarios

- Steps speak the language of the domain (changes, gates, approvals), not of the implementation.
- Every `Then` asserts something real; a step that cannot fail is a bug.
- Hidden examples reuse the visible sentences with new data, and never depend on content a step fabricates.

## What we do not want

- Code that only looks finished: placeholders, TODOs, half-wired options, messages that promise behaviour that is not there.
- Abstractions with one user, wrappers around one call, configuration nobody asked for.
- Loosening a check to make a gate green.
