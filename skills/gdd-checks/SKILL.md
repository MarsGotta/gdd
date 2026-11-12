---
name: gdd-checks
description: Writes and seals the cases of a change in Gate-Driven Development (GDD), a test for each scenario of its brief, or sets up a project's standing checks. Loaded only by the agent the GDD orchestrator launches for it, which never builds that change. Not for writing the change itself.
license: MIT
metadata:
  version: "0.6.0"
  contract: gdd/1
---

# Writing the checks

A gate that never closes is not a gate: it's a drawing on the wall. Your checks decide whether another agent's work gets through, so before anyone trusts one, you show it rejecting something done wrong, for the right reason. You never build the change, you don't launch other agents, and what you keep out of the builder's reach stays there.

## What you get

The brief and its scenarios in `features/`, already committed on `gdd-seal/<id>` and approved by the person when its color asks for it. After preparing or hardening you get the standing checks to set up instead, which have their own section below. Also the blueprint, for what the color asks for and the id of each check, the constitution, for the commands, and `<base>`, the commit of the main branch the change starts from.

When a change's color rose and you're adding the checks of its new color, work from the base and don't read what the builder already wrote: a check written with the code in sight ends up agreeing with the code.

## Climb the ladder

For every check, stop at the first step that works:

1. what the stack already brings: types, a linter rule, the test runner;
2. a mature open-source tool with a solid community or company behind it, taken as it is;
3. a small script of the project's own, only when nothing exists, following `references/scripts.md`;
4. a new or less proven tool, only with a notice (how old it is, who maintains it, its license, what it does with the data), and the person decides;
5. a reviewer or a person, when nothing deterministic works.

## What the color adds

The blueprint's `colors` say what each color asks for, and the blueprint wins. With the template's defaults:

- **Every change with a brief** gets a test for each scenario. Breaking it on purpose, the fourth step below, is what shows that each one closes.
- **Red** adds mutation testing on the code the change will touch, where the stack has a tool for it (the `mutation` check of the blueprint, scoped to those files). That code doesn't exist yet, so you leave the check ready: its command scoped to the brief's paths, and one run on what is already there to see it work. It runs at the gates, on what the builder wrote. A mutant that survives there is a hollow check, which comes back to you, or code no scenario asks for, which the builder takes out.
- **Hidden examples** exist only when the blueprint's `guarantee` is `platform` or `engine` and a color asks for `hidden`: more cases of the same scenarios, written by you, so that a solution made to measure for the visible ones fails. They go where the blueprint's `hidden` says, a place the builder's credentials can't read, never to a branch of the same repository.

## Each check, before you seal it

1. **Write it from its scenario.** A test cites its scenario's id as `gdd:<id>`, in its name or a comment, and goes through the real entry point (the command, the endpoint, the public function), not a helper behind it: in one run, a test checked a helper and the real entry point still let the bad value through.
2. **Watch it reject.** Run it on the bad case: for a new behavior, the code as it is, still without it; for a standing check, a bad case planted for it. It must fail on the assertion about the behavior, with a message that names what's wrong. A compile error, a missing import, a timeout or a skipped test is the wrong reason.
3. **Prove it can pass.** Run it on a correct version. If that takes a throwaway implementation, write it in your own worktree, out of the builder's reach, and delete it after: seeing it would give the solution away.
4. **Break it on purpose.** Make that correct version wrong (remove the line that does the work, return a constant, flip a comparison) and watch the check go red. If it stays green, it's hollow: one of the pilot's reviewers brought back the old function and all six tests still passed, and another replaced a hash with 64 zeros and the same 28 scenarios passed.
5. **More than one case.** Two or three examples per scenario, so a solution that only fits one of them isn't enough. Where a scenario states a rule that holds for every input (the total is never below zero) and the stack has a tool for it, check the rule over generated inputs too, with a fixed seed, so it gives the same answer every time, and keep each case it finds as one more example.

When you run a command, send its output to a file and read only its end.

## Sealing

Commit the checks on `gdd-seal/<id>`, on top of the brief and its scenarios, without any throwaway code. That commit is where the builder starts, and it never writes to it. Then report, so the orchestrator writes the seal to the log:

- the ref and its commit, and each check file with its blob hash (`git hash-object <file>`);
- the commit of the hidden examples, when the blueprint has them;
- for each check, the proof: the command, its exit code and its last lines, five at most, once rejecting and once passing.

From then on, nobody touches a sealed check. If one turns out to be wrong, fixing it is a new commit on the same ref, made by you or by a person, never by the builder, and if the fix changes what has to pass and the color asks for `brief-approval`, it goes back to the person.

## The standing checks

After preparing, you set up the machine checks the blueprint's `checks` name, once for the whole project and never once for each zone, and CI where there is none, running the constitution's commands as they are. `mutation` waits for the first red change. Each one goes at the lowest step of the ladder that works. A mature tool is taken as it is: you configure it, and you write no detector of your own beside it for what it might miss. Set the secret scan to report the file, the line and the kind of what it finds, never the secret itself, and to ignore the marks in the code that exempt a line from it, which any change could add.

Prove each check once, in a throwaway clone, and here that is its whole proof, one bad case and no more: plant it and watch the check fail and name the file and the line, then take it out and watch it pass. The bad case is the one the blueprint gives the check in `breaks`, which its gate's feature repeats under `Rule: It closes`. When there's none, choose the plainest thing the check exists to catch and report it, so that it goes into the blueprint with the person's yes. Nothing planted is committed. Seal what you set up on the ref you were given, `gdd-seal/standing-checks` the first time. Then `gdd-review` tries each check, and until they hold, no change starts.

For a zone's own gate, which hardening gave it or the person asked for, do the same for the check of each new `Rule:` in `gdd/gates/<zone>.feature`, built from its scenarios. There, what got past a mature tool in a real change becomes a rule in that tool's own configuration, or a script beside it when it takes none.

A secret never goes into your report, not even one you planted: say what kind it was and where it was. In one run, the log quoted a planted credential, the secret scan flagged the log, which is never rewritten, and three rounds went on it.

## Gotchas

Checks found already green, and hollow, in real runs:

- **The tests read their limits from the production code.** Raising the limit in production kept all twelve green. Write the expected numbers in the test.
- **A test double ignored its arguments,** so production could pass it anything, including a temporary folder inside the project.
- **A check that couldn't run reported success.** A runner skipped a file it didn't recognize and said "Files: 1/1"; a gate swallowed an error and printed `ok=true`. A check that can't run, runs nothing or loses an error must fail.
- **Temporary folders inside the repository inherited it.** The fixture found the parent repository and counted 0 events instead of 8, with exit code 0 and no warning.
- **A check compared files byte for byte with a fixed revision,** so it failed whenever another change edited them on purpose.
- **A check passed because the feature wasn't there.** A scenario about something that must not happen passed on a feature that didn't exist yet. Pair it with a check that the feature is there and does its job.
- **Unstable checks** don't get fixed by raising timeouts, running serially or retrying until green. Find the cause.

## Report

End your last message with this block, filled in. `result` is `sealed` or `stopped`. If you can't write a check that holds, start your last message with `CANNOT:` and the reason.

```gdd-report
skill: gdd-checks 0.6.0
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
```
