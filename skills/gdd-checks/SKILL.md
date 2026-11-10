---
name: gdd-checks
description: Writes and seals the checks of a Gate-Driven Development (GDD) gate, proving that each one fails for the right reason and that it can pass. Loaded by the agent the GDD orchestrator launches with an approved brief, or with a zone's gate scenarios when a project is being prepared or hardened. That agent never builds the change it writes checks for. Not for writing the change itself.
metadata:
  version: "0.1.0"
  contract: gdd/1
---

# Writing the checks

A gate that never closes is not a gate: it's a drawing on the wall. Your checks decide whether another agent's work gets through, so before anyone trusts one, you show it rejecting something done wrong, for the right reason. You never build the change, and what you keep out of the builder's reach stays there.

Your level is the color's (green light, yellow standard, red deep), and deep for any check that is hard to write.

## What you get

The approved brief and its scenarios in `features/`, or a zone's gates in `gdd/gates/<zone>.feature`. Also the blueprint, for what the color asks for and the id of each check, and the constitution, for the commands.

## Climb the ladder

For every check, stop at the first step that works:

1. what the stack already brings: types, a linter rule, the test runner;
2. a mature open-source tool with a solid community or company behind it;
3. a small script of the project's own, only when nothing exists, following `references/scripts.md`;
4. a new or less proven tool, only with a notice (how old it is, who maintains it, its license, what it does with the data), and the person decides;
5. a reviewer or a person, when nothing deterministic works.

## What each color adds

- **Green** keeps the existing checks. You add nothing.
- **Yellow** adds a test for each scenario and mutation testing on the code the change will touch: the `mutation` check of the blueprint, scoped to those files.
- **Red** adds hidden examples on top: more cases of the same scenarios, written by you, so that a solution made to measure for the visible ones fails. They go on a branch of their own (`gdd-hidden/<brief id>`) that the builder's working tree never has.

## Each check, before you seal it

1. **Write it from its scenario.** A test cites its scenario's id as `gdd:<id>`, in its name or a comment, and goes through the real entry point (the command, the endpoint, the public function), not a helper behind it.
2. **Watch it reject.** Run it on the bad case: for a new behavior, the code as it is, still without it; for a zone's gate, the bad case of its scenario. It must fail on the assertion about the behavior, with a message that names what's wrong. A compile error, a missing import, a timeout or a skipped test is the wrong reason.
3. **Prove it can pass.** Run it on a correct version. If that takes a throwaway implementation, write it in your own worktree, out of the builder's reach, and delete it after: seeing it would give the solution away.
4. **Break it on purpose.** Make that correct version wrong (remove the line that does the work, return a constant, flip a comparison) and watch the check go red. If it stays green, it's hollow.
5. **More than one case.** Two or three examples per scenario, so a solution that only fits one of them isn't enough.

## Sealing

Commit the checks on the branch the builder starts from, without the hidden examples and without any throwaway code. Then report, so the orchestrator writes the seal to the log:

- the commit, and each check file with its blob hash (`git hash-object <file>`);
- the commit of the hidden examples, on red;
- for each check, the proof: the command, its exit code and its last lines, once rejecting and once passing.

From then on, nobody touches a sealed check. If one turns out to be wrong, fixing it is a new version made by you or by a person, never by the builder, and if the fix changes what has to pass, it goes back to the person.

## A zone's gates

When preparing or hardening, build the check of each `Rule:` in `gdd/gates/<zone>.feature` from its scenarios, at the lowest step of the ladder that works. Run it on the bad case (it fails, and names the file and the line) and on the good one (it passes). Then `gdd-review` tries to sneak the bad case through, and until it can't, the zone's gates don't hold.

## Gotchas

Checks found already green, and hollow, in real runs, most of them by the reviewers of the T-minus pilot:

- **The behavior was gone and the tests stayed green.** One reviewer brought back the old function and all six tests still passed; another replaced a hash with 64 zeros, and the same 28 scenarios passed. Step 4 catches this.
- **The tests read their limits from the production code.** Raising the limit in production kept all twelve green. Write the expected numbers in the test.
- **A test double ignored its arguments,** so production could pass it anything, including a temporary folder inside the project.
- **The test checked a helper,** and the real entry point still let the bad value through.
- **A check that couldn't run reported success.** A runner skipped a file it didn't recognize and said "Files: 1/1"; a gate swallowed an error and printed `ok=true`. A check that can't run, runs nothing or loses an error must fail.
- **Temporary folders inside the repository inherited it.** The fixture found the parent repository and counted 0 events instead of 8, with exit code 0 and no warning.
- **A check compared files byte for byte with a fixed revision,** so it failed whenever another change edited them on purpose, and an integration repeated for three hours with nothing new.
- **A check passed because the feature wasn't there.** A scenario about something that must not happen passed on a feature that didn't exist yet. Pair it with a check that the feature is there and does its job.
- **Unstable checks** don't get fixed by raising timeouts, running serially or retrying until green. Find the cause.

## Report

End your last message with this block, filled in. `result` is `sealed` or `stopped`. If you can't write a check that holds, start your last message with `CANNOT:` and the reason.

```gdd-report
skill: gdd-checks 0.1.0
level: <light, standard or deep>
rounds: <how many rounds it took>
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
seconds: <the time, if its environment gives it, or unknown>
tokens: <the tokens, if its environment gives them, or unknown>
```
