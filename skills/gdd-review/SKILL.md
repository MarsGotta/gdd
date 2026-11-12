---
name: gdd-review
description: Reviews a change in a Gate-Driven Development (GDD) project with a clean context, or tries new standing checks before any change relies on them. Loaded only by the agent the GDD orchestrator launches for it. Never for building or approving, which belongs to people.
license: MIT
metadata:
  version: "0.7.1"
  contract: gdd/1
---

# Reviewing a change

Your job is to make it fail. You judge the change, not the story about it: what the builder reports is a claim, and it only serves to compare with what you find. You don't build, approve or launch other agents.

If you were given new standing checks instead of a change, open `references/standing-checks.md`, in this skill's folder, and follow it instead of what comes next.

## What you read

Work in a fresh clone of the repository, at the change's head, never in the tree you were launched in: a worktree shares its refs and its configuration with the builder's.

- The change: `git diff <start> HEAD`, what the builder did. `<base>` and `<start>` are the commits the orchestrator gave you. `<start>` holds everything that judges the change, and it's the base when nothing was sealed.
- From `<start>`, never from the change's head: the blueprint, the gates of its zones, the constitution and, when there is one, the brief and its scenarios. A change can edit any of them.
- The log of the change, with its approvals and its seal.

## Every change

1. **The color, on what was touched.** For each zone, `git diff --name-only <start> HEAD -- <pathspec>`, from the root of the repository; then once more excluding every zone to find what none covers, which is never green: judge it as the brief did, against what the blueprint marks as delicate, and take the blueprint's word where it says what that counts as. A path written `':(glob)<pattern>'` is excluded as `':(exclude,glob)<pattern>'`: `:(exclude)` put in front of `:(glob)` excludes nothing and gives no error. Then judgment, on what paths miss: code a delicate zone imports, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files, behavior someone would see that a green change added, and anything that can't be undone, which is never green. The color can only go up.
2. **Nothing that judges was touched.** That is the `seal` check, and you run it yourself, because a change can edit the very check that compares it:
   - `git diff --exit-code <start> HEAD -- gdd features AGENTS.md CLAUDE.md .agents .claude <sealed paths> ':(exclude)gdd/log' ':(exclude)gdd/backlog.md'` shows nothing. Compare with the commit, never with the name `gdd-seal/<id>`, which can be moved. The sealed paths are in the log, each with its blob hash: `git hash-object <file>` gives the same one in your clone.
   - `git diff --name-only <start> HEAD -- gdd/log gdd/backlog.md` lists nothing: those are the orchestrator's, and it adds them after you.
   - `git diff --name-only <base> <start>` lists only the brief, its scenarios and the sealed checks, and what the main branch brought if it was merged into the seal.
   - When there is a brief, at `<start>` it has the blob hash of the last `brief` step in the log (`git rev-parse <start>:<path>`) and, when the color asks for `brief-approval`, each scenario file has the one of its last approval.
   - No test that was already there was weakened, skipped or deleted, and what surrounds the tests changed only for a reason the brief or the request gives: the test script in the package manifest, test configuration and setup files, hooks, CI workflows.
3. **The checks that applied.** A check with a `when` in the blueprint applies when `git diff --name-only <start> HEAD -- <pathspec>` prints something for its paths. Say which checks of the color didn't apply, and why: one that applied and wasn't run is a `fix`.

## Then, by the color you were launched for

- **Its checks include `review`** (yellow and red, with the template's defaults), or a zone the change touches has a gate for the reviewer: open `references/full-review.md`, in this skill's folder, read it whole and do all of it. If you can't open it, say so and stop: steps 1 and 2 aren't the review of that color. A script of the project's own is judged by `references/scripts.md`.
- **Otherwise** (green, with those defaults): steps 1 and 2 are the whole review, even if you found a higher color: the review of that color is another launch, at its level. Here `pass` means nothing that judges was touched.

## Your verdict

The first line of your answer is exactly one of these words, with nothing before it, because the orchestrator reads it as it is:

- `pass`: you ran it and it works. Minor issues can remain; list them after a line `LATER:`.
- `fix`: there's a real error the builder has to fix: it breaks what has to pass, or it leaves the code worse to change. Write a line `NOTES FOR THE BUILDER:` and, under it, concrete corrections in under 200 words, without writing the solution.
- `stop`: it can't go in like this: it touched what judges it, or it opens what it shouldn't, runs foreign code, exposes a secret, loses data or breaks an existing project.

The second line is `COLOR: <color>`, with the zones it touched, anything no zone covers and, if it rose, why. Then what you ran and what came out, a line each, and what fails, with its file and line. Without that, a `pass` on the full review doesn't count. Number your findings (`f1`, `f2`). Ask for `fix` only for errors that matter: an agent fixes every nitpick it gets, and each one opens another round.

## Report

End your last message with this block, filled in. `result` is your verdict.

```gdd-report
skill: gdd-review 0.7.1
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
```
