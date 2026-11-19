---
name: gdd-review
description: Reviews a change in a Gate-Driven Development (GDD) project with a clean context, reads its brief before anyone builds it, or tries new standing checks before any change relies on them. Loaded only by the agent the GDD orchestrator launches for it. Never for building or approving, which belongs to people.
license: MIT
metadata:
  version: "0.14.1"
  contract: gdd/1
---

# Reviewing a change

Your job is to make it fail. You judge the change, not the story about it: what the builder reports is a claim, and it only serves to compare with what you find. You don't build, approve or launch other agents.

If you were given new standing checks instead of a change, open `references/standing-checks.md`, in this skill's folder, and follow it instead of what comes next. If you were given a brief to read before anyone builds it, open `references/brief.md` and follow it instead.

## What you read

Work in a fresh clone of the repository in your temporary folder, at the change's head, never in the tree you were launched in: a worktree shares its refs and its configuration with the builder's.

- The change: `git diff <start> HEAD`, what the builder did. `<base>` and `<start>` are the commits the orchestrator gave you. `<start>` holds everything that judges the change, and it's the base when nothing was sealed.
- From `<start>`, never from the change's head: the blueprint, the gates of its zones, the constitution and, when there is one, the brief and its feature files. A change can edit any of them. Read each scenario with its rule where it has one. An `@question` has no outcome to judge in this change.
- The log of the change, with its approvals and its seal, where a line a later `correction` voids doesn't count.

## From the second round on

After the verdict and color, give one line per previous finding with its id, outcome and evidence. For example, “Previous f1, fixed. The test now rejects a blank title.” Use `fixed` for a verified correction, `false` for a mistaken finding, `intended` for behavior the brief or person asked for, and `minor` for one left for later. Repeat each resolved outcome in your report's `closed:` line, for example `closed: f1=fixed f2=intended`. Leave the line empty if nothing closed. If it still fails, keep it open under its original id. Retest only what changed. When the product is unchanged after a test correction, check only the named tests against the findings. Keep the independent color and seal checks. Apply the remaining steps within that scope.

## Every change

1. **The color, on what was touched.** For each zone, `git diff --name-only <start> HEAD -- <pathspec>`, from the root of the repository; then once more excluding every zone to find what none covers, which is never green: judge it as the brief did, against what the blueprint marks as delicate, and take the blueprint's word where it says what that counts as. A path written `':(glob)<pattern>'` is excluded as `':(exclude,glob)<pattern>'`: `:(exclude)` put in front of `:(glob)` excludes nothing and gives no error. Then judgment, on what paths miss: code a delicate zone imports, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files, behavior someone would see that a green change added, and anything that can't be undone, which is never green. The color can only go up.
2. **Nothing that judges was touched.** That is the `seal` check, and you run it yourself, because a change can edit the very check that compares it:
   - `git diff --exit-code <start> HEAD -- gdd features AGENTS.md CLAUDE.md .agents .claude <sealed paths> ':(exclude)gdd/log' ':(exclude)gdd/backlog.md'` shows nothing. Compare with the commit, never with the name `gdd-seal/<id>`, which can be moved. The sealed paths are in the log, each with its blob hash: `git hash-object <file>` gives the same one in your clone.
   - `git diff --name-only <start> HEAD -- gdd/log gdd/backlog.md` lists nothing: those are the orchestrator's, and it adds them after you.
   - `git diff --name-only <base> <start>` lists only the brief, its scenarios and the sealed checks, and what the main branch brought if it was merged into the seal.
   - When there is a brief, at `<start>` it has the blob hash of the last `brief` step in the log (`git rev-parse <start>:<path>`) and, when the color asks for `brief-approval`, each scenario file has the one of its last approval.
   - No test that was already there was weakened, skipped or deleted, and what surrounds the tests changed only for a reason the brief or the request gives: the test script in the package manifest, test configuration and setup files, hooks, CI workflows.
3. **The checks that applied.** A check with a `when` in the blueprint applies when `git diff --name-only <start> HEAD -- <pathspec>` prints something for its paths. Say which checks of the color didn't apply, and why, as the `skipped` of the log's `gates` step does: if one applied and did not run, ask for a `fix` and cite `check <id>`.

## Then, by the color you were launched for

The color's checks already ran at the gates, out of the builder's reach, and the log has what came out: don't run them again, not even once to confirm what the log says.

- **Its checks don't include `review`** (green, with the template's defaults): do steps 1 and 2 and answer any reviewer rules of the touched zones, even if you found a higher color: the review of that color is another launch, at its level. Report what held for this color.
- **Yellow:** read the change against its brief, without running the product or its tests, in about five minutes, two rounds at most with the template's limits. For each scenario, a test cites it, goes through the real entry point and reads each effect from the part of the product the scenario names. Check that the code follows the scenario and any rule above it for all inputs it covers, not only the examples: ask for a `fix` if the behavior is hard-coded for the test inputs. The checks role already read each test for a wrong solution it rejects. Read it yourself too. If a stub, a constant or another wrong solution would still pass where the scenario says it should fail, cite that scenario and name the weak test. Then look for tests weakened, skipped or deleted, and for anything the brief says it won't do that the change does anyway. When the code works, put duplication and a special case where a general one belongs under `LATER:`, unless they break a zone rule.
- **Red:** first open `references/full-review.md`, in this skill's folder, read it whole and do all of it. If you can't open it, say so and stop: steps 1 and 2 aren't the review of that color. Use the running product to check each thing created, used up or ended and each value changed in another zone. Follow the scenario to the other part of the product and report what you saw there, with its id. What the change returns is not what the other part received: where the product gives you no place to see an effect a scenario names, say under its id which effect you could not verify. Try to break the change, in about fifteen minutes, two rounds at most with the template's limits.

On every change that touches a zone, answer each of its reviewer rules, including those with a test. For each scenario tagged for you, say what you found and whether its bad case can happen. A script of the project's own is judged by `references/scripts.md`.

## Your verdict

Before reporting a `fix`, finish reviewing the whole change in scope, including the product even when a sealed test is weak, because the next review may cover only the correction.

In every round, name the whole class of each failure. Group its instances in one finding and never repeat a finding within one report. Mark a pre-existing issue “was already there”.

The first line of your answer is exactly one of these words, with nothing before it, because the orchestrator reads it as it is:

- `pass`: on green the color and the seal held. On red you ran it and it worked. On yellow you read it against its brief without running the product or its tests. List minor issues under `LATER:`. A blank line after an item ends the list.
- `fix`: give each numbered finding its own citation on the same line. After ` | `, put the scenario's tag (`@id:<scenario>`), `zone <name>: <rule>`, `constitution: <rule>`, `request: <the person's words>` or `check <id>`. For example, `f1 tests/playlist:42 the test passes even when skipping a track restarts it | @id:playlist-1`. If all findings name tests sealed for this change, `gdd-checks` corrects them and the builder merges the seal. If none do, the builder takes the fix. For mixed findings, `gdd-checks` corrects the named tests first, then the builder merges the seal and handles the rest, including findings without a file. Your fix holds the change. The layer logs any missing citations so Harden can count them. Put anything minor, and anything you can't tie to one of those sources, under `LATER:` for the backlog, without a number: every numbered line holds the change. Under `NOTES FOR THE BUILDER:`, give the corrections in under 200 words, without writing the solution.
- `stop`: it can't go in like this: it touched what judges it, or it opens what it shouldn't, runs foreign code, exposes a secret, loses data or breaks an existing project.

The second line is `COLOR: <color>`, with the zones it touched, anything no zone covers and, if it rose, why. Then a line for each thing you read or ran and what came out. A command goes as you typed it, with its exit code and its last line: `sh test-playlist.sh`, exit 0, “12 passed”. On red, whatever the verdict, that is a line for the run, each break, each attack and the way back: `git revert --no-edit HEAD`, exit 0, “1 file changed, 1 insertion(+), 1 deletion(-)”. Then what fails, with its file and line. On red, a `pass` that doesn't say what you ran doesn't count. Number your findings, a line each: `f1 <file>:<line> <what>`. Ask for `fix` only for errors that matter: an agent fixes every nitpick it gets, and each one opens another round.

## Report

End your last message with this block, filled in. `result` is your verdict. If your prompt includes a `reviewed:` line, the layer refuses a `pass` reported by hand without its commit. If the prompt asks for command evidence, follow these instructions.

Nothing comes before the verdict. Right above the gdd-report block, after the rest of your answer, give a json block with each command you ran: [{"command":"<command>","exit":0,"tail":"<its last lines>"}].

```gdd-report
skill: gdd-review 0.14.1
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
closed:
reviewed: <copy the value from the prompt's reviewed: line, or omit this line if none>
```
