---
name: gdd-harden
description: Hardens a Gate-Driven Development (GDD) project after something slipped through its gates or keeps repeating. Loaded by the GDD orchestrator, with the person, when a failure reaches the main branch or production, when a reviewer catches what the machine checks passed, or when the logs show the same failure coming back. Not for fixing the failure itself.
license: MIT
metadata:
  version: "0.1.0"
  contract: gdd/1
---

# Hardening

Gates with cracks get hardened. When something slips through, fixing it isn't enough: what it would have taken to catch it stays as a check, so that it can't get in that way again. Over time, the method interrupts the person less. This runs deep.

## What counts

- **What slips through:** a failure that got through every gate and was found later, by a person, in production, or by a reviewer where a machine check should have caught it.
- **What repeats:** the same finding, the same stop or the same reason for raising a color, in several logs.

Before anything else, ask someone who knows that part of the product to confirm the failure is real and where it was.

## Finding the crack

Read the brief and the log of the change that let it through (`gdd/log/<id>.jsonl`), and find which gate should have caught it and what that gate was missing:

- no gate covered that behavior;
- a check passed without the behavior, because it was hollow;
- the path fell in no zone, or in a zone with too low a color;
- judgment didn't raise the color, or raised it for the same reason again and again;
- the gate didn't get the context it needed;
- the reviewer had no question about it on its exam.

## What you propose

- **The scenario that would have caught it,** with its bad case first, in `gdd/gates/<zone>.feature` (`assets/gates.feature`), tagged with the check that will cover it.
- **The check.** If a machine can check it, it goes to the blueprint and `gdd-checks` builds it, climbing the ladder from what the stack already brings. If no machine can, its scenario is tagged with the reviewer check, and it becomes a question on the reviewer's exam.
- **A pathspec or a zone,** when the same reason for raising a color keeps coming back: what judgment finds today, git checks tomorrow.
- **Never a paragraph.** A new line in the constitution or in `AGENTS.md` costs at every gate and checks nothing.

The person approves the change to the blueprint. Then `gdd-checks` builds the check, and `gdd-review` tries to fool it before it counts. Fixing the failure itself is a normal change, through `gdd-brief`.

## The reviewer's exam

Every confirmed failure that slipped through becomes a question on the reviewer's exam: its scenario in the zone's gates. When the reviewer's model, its instructions or its tools change, run the whole exam through it again. If its score drops, the change doesn't stay, because catching real failures counts for more than saving rounds.

## Reading the logs

When asked, or now and then, read `gdd/log/` to compare the project with itself:

- **Per gate:** how often it fires, how many of those were real failures, what slipped past it and what it costs. If it always fires for the same thing, that thing can be caught earlier.
- **Per zone:** if its color rises often, it's badly marked; if almost everything comes out red, the blueprint marks too much.
- **Per approval:** an approval that lasts seconds is approving blind.
- **Per round:** if an agent gives up or disputes the same thing every time, or the reviewer takes long to pass a change, the brief or the checks are wrong.

## Loosening

Retiring a gate that no longer adds anything, dropping an approval a color asked for, lowering a color or recording new debt all change the blueprint, so the person approves them, and they never happen on their own. Before deciding a gate adds nothing, check that it still closes: a gate that never fires can mean quality, or that it detects nothing.

## Gotchas

- In the T-minus pilot, the same tests failed now and then without anything changing, and the gate rejected good changes for it. A check that shows up as unstable three times or more gets a small change of its own that makes it stable without weakening it: no higher timeouts, no skipping.

## Report

End your last message with this block, filled in. `result` is `proposed` or `approved`.

```gdd-report
skill: gdd-harden 0.1.0
level: <light, standard or deep>
rounds: <how many rounds it took>
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
seconds: <the time, if its environment gives it, or unknown>
tokens: <the tokens, if its environment gives them, or unknown>
```
