---
name: gdd-checks
description: Writes and seals the cases of a change in Gate-Driven Development (GDD), a test for each scenario of its brief, or sets up a project's standing checks. Loaded only by the agent the GDD orchestrator launches for it, which never builds that change. Not for writing the change itself.
license: MIT
metadata:
  version: "0.13.1"
  contract: gdd/1
---

# Writing the checks

A gate that never closes is not a gate: it's a drawing on the wall. Your checks decide whether another agent's work gets through, so before anyone trusts one, you show it rejecting something done wrong, for the right reason. You never build the change, you don't launch other agents, and what you keep out of the builder's reach stays there.

Save command output to a file and read only its end. Don't print unchanged files again. Each repeat fills the next round's context.

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

- **Every change with a brief** gets a test for each scenario, and nothing no scenario asks for: no extra sizes, repetitions or cases. What looks missing goes in your report under `MISSING:`, a line each, for the orchestrator to take to the brief, never into a test.
- **Yellow** takes steps 1, 2 and 5 below, with two examples per scenario, and generated inputs only where the scenario states a rule for every input. Read the tests to find weak assertions. Write no throwaway implementation and break nothing on purpose. In one yellow change a throwaway solution was broken by hand in 36 ways, which the person took for a mutation gate.
- **Red** takes all five steps.
- **The standing checks** you set up after preparing keep every step of their own section below, whatever color they serve: they judge every change.
- **A check that waits,** one whose `standing-checks-<check id>` is still `todo` in the backlog: when this change is the first to bring what its `when` names, set up with your tests the runner or the configuration it needs, sealed with them, so that the change's gates run it for real.
- **Mutation testing,** once hardening added it to a color (the `mutation` check of the blueprint), judges what the change adds or changes between `<start>` and its head, never the files around it: code that got in under a lighter color never went through it. It runs at the gates, before the review, on what the builder wrote. A mutant that survives there is a hollow check, which comes back to you, or code no scenario asks for, which the builder takes out.
- **Hidden examples** exist only when the blueprint's `guarantee` is `platform` or `engine` and a color asks for `hidden`: more cases of the same scenarios, written by you, so that a solution made to measure for the visible ones fails. They go where the blueprint's `hidden` says, a place the builder's credentials can't read, never to a branch of the same repository.

## Each check, before you seal it

On every color with cases, read each test against its scenario before sealing. Name a plausible wrong solution and point to the inputs and assertion that would reject it. Try a constant, the same branch every time, two swapped fields, the example numbers alone or a mouse-only button. If you can't name one, or that solution would still pass, strengthen the test and read it again. In one run a test checked an expired discount code only after expiry. Rejecting the code on every date would pass. A case on a valid date catches that mistake.

1. **Write it from its scenario.** A test cites its scenario's id exactly as `gdd:<id>`, in a comment where its name can't hold it, and goes through the real entry point (the command, the endpoint, the public function), not a helper behind it: in one run, a test checked a helper and the real entry point still let the bad value through. For each thing created, used up or ended, and each value changed in another zone, read the result through the other part of the product named in the scenario. A payment receipt can say “paid” while the catalog still offers the last unit. If the scenario leaves the outcome, timing or viewpoint open, report it under `MISSING:`.
2. **Watch it reject.** Run it on the bad case: for a new behavior, the code as it is, still without it; for a standing check, a bad case planted for it. It must fail on the assertion about the behavior, with a message that names what's wrong. A compile error, a missing import, a timeout or a skipped test is the wrong reason. When the change creates its entry point, the error that says it doesn't exist yet is the right one: write no stub to turn it into an assertion.
3. **Prove it can pass, on red.** Run it on a correct version. If that takes a throwaway implementation, write it in your own worktree, out of the builder's reach, and delete it after: seeing it would give the solution away. On yellow, run it only on a correct version that already exists, or one that costs a single line and isn't the behavior the scenario asks for. Write no version of that behavior, not even one line in a scratch copy: your report says for that check "seen rejecting, not seen passing".
4. **Break it on purpose, on red, and never on yellow.** Make that correct version wrong once for each check, with the break that removes the work (remove the line that does it, return a constant, flip a comparison), and watch the check go red. If it stays green, it's hollow: strengthen it and break it again, the only time a second break is worth it. One of the pilot's reviewers brought back the old function and all six tests still passed, and another replaced a hash with 64 zeros and the same 28 scenarios passed.
5. **More than one case.** Two examples per scenario on yellow, two or three on red, so a solution that only fits one of them isn't enough. Where a scenario states a rule that holds for every input (the total is never below zero) and the stack has a tool for it, check the rule over generated inputs too, with a fixed seed, so it gives the same answer every time, and keep each case it finds as one more example.

## Sealing

Commit the checks on `gdd-seal/<id>`, on top of the brief and its scenarios, without any throwaway code. That commit is where the builder starts, and it never writes to it. Then report, for the layer, which reads the seal and its hashes from git:

- the ref and its commit;
- the commit of the hidden examples, when the blueprint has them;
- the proofs in a `json` list before your report block. Give each scenario test its own rejecting entry, even if several ran together. Use `test` for its name and `wrong` for the solution you ruled out by reading it, e.g. `[{"check":"tests","test":"accepts a valid code","wrong":"rejects the code on every date","case":"rejects","command":"<command>","exit":1,"tail":"<last lines, five at most>"}]`. The command, exit and tail describe the actual run. Include a `passes` entry where you saw it pass, otherwise say "seen rejecting, not seen passing". For standing checks, report the planted case and the run with it removed;
- under `MISSING:`, what the scenarios seem to leave out, if anything.

From then on, nobody touches a sealed check. If one turns out to be wrong, fixing it is a new commit on the same ref, made by you or by a person, never by the builder, and if the fix changes what has to pass and the color asks for `brief-approval`, it goes back to the person.

When you're launched for a new version of the brief that changed only its prose, the checks you sealed before may still cover it. Run them against it, and if they hold, commit nothing: report `sealed` with their proofs, and the layer records them as they stand. A new scenario, a changed check or a higher color always takes a new seal.

## The standing checks

After preparing, you set up the machine checks the blueprint's `checks` name, once for the whole project and never once for each zone, and CI where there is none, running the constitution's commands as they are. Each one goes at the lowest step of the ladder that works. A mature tool is taken as it is: you configure it, and you write no detector of your own beside it for what it might miss. When the project builds for the web, open `references/web.md` before you set up its tests: the traps met setting them up, and what works. Set the secret scan to report the file, the line and the kind of what it finds, never the secret itself, and to ignore the marks in the code that exempt a line from it, which any change could add.

In a project with no code yet, set up only `seal` and `secrets`, and CI running them. Every other check waits in the blueprint with its `when`, and nobody is asked to demonstrate a check on code that does not exist: that isn't a `CANNOT:`. Its own item, `standing-checks-<check id>`, proves it later, on the code the first change brought.

Prove each check once, in a throwaway clone, and here that is its whole proof, one bad case and no more: plant it and watch the check fail and name the file and the line, then take it out and watch it pass. The bad case is the one the blueprint gives the check in `breaks`, which its gate's feature repeats under `Rule: It closes`. When there's none, choose the plainest thing the check exists to catch and report it, so that it goes into the blueprint with the person's yes. Nothing planted is committed. Seal what you set up on the ref you were given, `gdd-seal/standing-checks` the first time. Then `gdd-review` tries each check, and until they hold, no change starts.

Two kinds of check need more care, because an empty project proves nothing about them:

- **A check that not every color asks,** like `mutation` once hardening adds it. Set it up with the rest, to judge only what a change adds or changes between `<start>` and its head. If it judged the whole code, the first change of that color would answer for everything the lighter ones shipped. Prove it on code: plant a small piece with a weak test inside a commit and watch it fail, then plant the same outside that commit and watch it pass.
- **A check that closes at a number,** a score or a budget. Take the tool's own default, looked up on its own page and never from memory, or what the code as it is reaches, and report it, so that it goes into the blueprint with the person's yes. The highest number isn't the strictest gate: it's one no change gets through, because some mutants change nothing anyone could observe.

For each approved zone gate with a machine check, do the same for the check of each new `Rule:` in `gdd/gates/<zone>.feature`, built from its scenarios. There, what got past a mature tool in a real change becomes a rule in that tool's own configuration, or a script beside it when it takes none.

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
- **A check tried on an empty project.** Mutation over the whole code, at its highest score, passed its trial when there was no code. The first red change found it failing on what a yellow one had shipped, and never got through it.

## Report

End your last message with this block, filled in. `result` is `sealed` or `stopped`. If you can't write a check that holds, start your last message with `CANNOT:` and the reason.

Many scenarios are no reason to stop: write and seal them one after another, and stop only on one you can't check, naming it by its `@id`, or naming the file you need. The layer refuses a stop that names neither: in one run, five rounds stopped on "the eleven scenarios" and wrote no test. Don't read `gdd/log/`: what you need is in your prompt, and an earlier round's stop isn't yours. Those five rounds had read the stops before them.

```gdd-report
skill: gdd-checks 0.13.1
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
```
