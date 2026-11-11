---
name: gdd-harden
description: Hardens a Gate-Driven Development (GDD) project after something slipped through its gates or keeps repeating. Loaded by the GDD orchestrator, with the person, when a failure gets past the gates, when a reviewer catches what the machine checks passed, or when the logs show the same one coming back. Not for fixing the failure itself.
license: MIT
metadata:
  version: "0.2.1"
  contract: gdd/1
---

# Hardening

Gates with cracks get hardened. When something slips through, fixing it isn't enough: what it would have taken to catch it stays as a check, so that it can't get in that way again. You do it yourself, as the orchestrator, with the person, and it needs the deep level.

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
- **The check.** If a machine can check it, it goes to the blueprint and `gdd-checks` builds it, climbing the ladder from what the stack already brings. If no machine can, its scenario is tagged with the reviewer's check, and it becomes a question of the reviewer's exam.
- **A pathspec or a zone,** when the same reason for raising a color keeps coming back: what judgment finds today, git checks tomorrow.
- **A removal,** with the same evidence as an addition. When a gate has gone without catching anything for as long as the blueprint says would make the team simplify, propose retiring it: how often it ran, what it caught, and what it would let through from then on.
- **Never a paragraph.** A new line in the constitution or in `AGENTS.md` costs at every gate and checks nothing.

You write the proposal as a change to `gdd/blueprint.md` and `gdd/gates/`, and the person approves it: a pull request, or a commit of theirs. Then `gdd-checks` builds the check, and `gdd-review` tries to fool it before it counts. Fixing the failure itself is a normal change, through `gdd-brief`, and so is anything this leaves in the backlog.

## The reviewer's exam

Every confirmed failure that got past a reviewer becomes a question, in two places. Its scenario, tagged with the reviewer's check in the zone's gates, is asked of every later change there. And the change that carried it goes to `gdd/exam.md` (`assets/exam.md`), as the two commits of the main branch it shipped between, so it can be shown to a reviewer again.

Run the exam when the reviewer's model, its instructions or its tools change, and once before that, to have a mark to compare with. For each row, launch `gdd-review` on a fresh clone at the row's `To`, with its `From` as `<start>` and the brief that change had, and nothing that tells it this is an exam. It catches the question when its verdict isn't `pass` and it names what the row says. Keep a good change or two among the rows, which it has to pass: a reviewer that rejects everything would get full marks. Write an `exam` step to `gdd/log/blueprint.jsonl`, with who was examined, how many it caught and which it missed. If the new reviewer catches fewer than the one it replaces, the change of reviewer doesn't stay, because catching real failures counts for more than saving rounds.

## Reading the logs

When asked, or now and then, read `gdd/log/` to compare the project with itself. Count before you read: the counts come from `grep`, and you open only the logs that stand out.

- `grep -cE '"event": ?"ask"' gdd/log/*.jsonl`: how often each change stopped a person.
- `grep -lE '"verdict": ?"fix"' gdd/log/*.jsonl`: the changes a reviewer sent back.
- `grep -hoE '"outcome": ?"[a-z]*"' gdd/log/*.jsonl | sort | uniq -c`: how the reviewers' findings ended.

What to look for:

- **Per gate:** how often it fires, how many of those were real failures, what slipped past it and what it costs. If it always fires for the same thing, that thing can be caught earlier.
- **Per zone:** if its color rises often, it's badly marked; if almost everything comes out red, the blueprint marks too much.
- **Per approval:** the time from its `ask`. One that comes seconds later is approving blind, and one that waits for hours means nobody was told.
- **Per change:** the time from its first event to its `ship`, and how many times it asked for a person.
- **Per reviewer:** how many of its findings ended as `false` or `minor`. That is its noise.
- **Per round:** if an agent gives up or disputes the same thing every time, or the reviewer takes long to pass a change, the brief or the checks are wrong.

Compare it with the blueprint's baseline, and with what the team wrote there that would make it simplify.

## Loosening

Retiring a gate that no longer adds anything, dropping an approval a color asked for, lowering a color or recording new debt all change the blueprint, so the person approves them, and they never happen on their own. Before deciding a gate adds nothing, check that it still closes: a gate that never fires can mean quality, or that it detects nothing.

## Gotchas

- In the T-minus pilot, the same tests failed now and then without anything changing, and the gate rejected good changes for it. A check that shows up as unstable three times or more gets a small change of its own that makes it stable without weakening it: no higher timeouts, no skipping.

When you close, write the `harden` step to `gdd/log/blueprint.jsonl`, with the change that let the failure through: its `result` is `proposed` or `approved`.
