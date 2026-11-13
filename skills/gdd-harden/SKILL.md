---
name: gdd-harden
description: Hardens a Gate-Driven Development (GDD) project in both directions, adding a check after something slipped through its gates or keeps repeating, and relaxing or retiring a gate that adds nothing. Loaded by the GDD orchestrator, with the person, when a failure gets past the gates, when the logs show it coming back, or when a gate costs more than it catches. Not for fixing the failure itself.
license: MIT
metadata:
  version: "0.11.1"
  contract: gdd/1
---

# Hardening

Gates with cracks get hardened. When something slips through, fixing it isn't enough: what it would have taken to catch it stays as a check, so that it can't get in that way again. Hardening is also knowing when to reduce: a gate that no longer catches anything, or that costs more than it protects, is relaxed or retired, on the same evidence. The two keep the balance. You do it yourself, as the orchestrator, with the person, and it needs the deep level. It never starts by itself, and only between changes: a change leaves one proposal at most in the backlog, for the person once it ends, and hardening starts when they take it up or ask. In one run, hardening started inside a request, and three more red changes opened on protecting the database's files.

## What counts

- **What slips through:** a failure that got through every gate and was found later, by a person, in production, or by a reviewer where a machine check should have caught it.
- **What repeats:** the same finding, the same stop or the same reason for raising a color, in several logs.
- **What no longer adds anything:** a check, a question of the reviewer's or a row of its exam that the person says is in the way, or that reaches the figure the blueprint gives for simplifying. Go to Reading the logs, and then to what you propose.

For a failure, before anything else, ask someone who knows that part of the product to confirm it is real and where it was.

## Finding the crack

Read the brief and the log of the change that let it through (`gdd/log/<id>.jsonl`), and find which gate should have caught it and what that gate was missing:

- no gate covered that behavior;
- a check passed without the behavior, because it was hollow;
- the path fell in no zone, or in a zone with too low a color;
- judgment didn't raise the color, or raised it for the same reason again and again;
- the gate didn't get the context it needed;
- the reviewer had no question about it on its exam.

## What you propose

- **The scenario that would have caught it,** with its bad case first, in `gdd/gates/<zone>.feature` (`assets/gates.feature`), tagged with the check that will cover it. This is where a zone gets gates of its own: when preparing, only if the person asks for one.
- **The check.** If a machine can check it, it goes to the blueprint and `gdd-checks` builds it, climbing the ladder from what the stack already brings, and its bad case goes under `Rule: It closes` in the feature of its gate. If no machine can, its scenario is tagged with the reviewer's check, and it becomes a question of the reviewer's exam.
- **Mutation testing, or a threshold of coverage,** when the tests proved weak: a reviewer found one that passed without its behavior, or a failure got past tests that ran its lines. Propose it for the color where that happened, before the review, at its tool's own default, looked up on the tool's own page and never from memory, and never the highest.
- **A pathspec or a zone,** when the same reason for raising a color keeps coming back: what judgment finds today, git checks tomorrow.
- **A removal,** with the same evidence as an addition. When a check, a question of the reviewer's or a row of its exam reaches the figure the blueprint says would make the team simplify, plant its bad case and see it still rejected, and then propose retiring it: how often it ran, what it caught, what it cost and what it would let through from then on.
- **Never a paragraph.** A new line in the constitution or in `AGENTS.md` costs at every gate and checks nothing.

You write the proposal as a change to `gdd/blueprint.md` and `gdd/gates/`, and the person approves it: a pull request, or a commit of theirs, never one you make or offer to make for them. Until then it is a proposal, and you say so: nothing is added or retired yet. Then the check gets built and tried before it counts, as the backlog item `gates-<zone>`, the way the standing checks were: the `gdd` skill's reference `standing-checks.md` says how. Tell the person what that trial leaves under `LATER:` and put it in the backlog, without waiting: the gate stays as they approved it unless they change it. Fixing the failure itself is a normal change, through `gdd-brief`, and so is anything this leaves in the backlog.

## The reviewer's exam

Every confirmed failure that got past a reviewer becomes a question, in two places. Its scenario, tagged with the reviewer's check in the zone's gates, is asked of every later change there. And the change that carried it goes to `gdd/exam.md` (`assets/exam.md`), as the two commits of the main branch it shipped between, so it can be shown to a reviewer again.

Run the exam when the reviewer's model, its instructions or its tools change, and once before that, to have a mark to compare with. For each row, launch `gdd-review` on a fresh clone at the row's `To`, with its `From` as `<start>` and the brief that change had, and nothing that tells it this is an exam. It catches the question when its verdict isn't `pass` and it names what the row says. Keep a good change or two among the rows, which it has to pass: a reviewer that rejects everything would get full marks. Write the `exam` step: `gdd step blueprint exam --level <level> --model <model> --asked <n> --caught <n>`, and a `--missed <id>` for each it missed. If the new reviewer catches fewer than the one it replaces, the change of reviewer doesn't stay, because catching real failures counts for more than saving rounds.

## Reading the logs

When asked, or now and then, compare the project with itself. Count before you read: `gdd metrics --all --summary` gives the counts, and `gdd metrics --all --format csv --out <folder>` the tables behind them, one row per change, step, check run, question to the person, finding and call of the layer. Open only the logs that stand out.

What to look for:

- **Per gate:** how often it fires, how many of those were real failures, what slipped past it and what it costs: the seconds of its checks in `checks`, the seconds and the tokens of the reviewer's reports in `steps`, and the wait after each `ask` in `asks`. If it always fires for the same thing, that thing can be caught earlier.
- **Per zone:** if its color rises often, it's badly marked; if almost everything comes out red, the blueprint marks too much.
- **Per approval:** the time from its `ask`. One that comes seconds later is approving blind, and one that waits for hours means nobody was told.
- **Per change:** the time from its first event to its `ship`, and how many times it asked for a person.
- **Per reviewer:** how many of its findings ended as `false` or `minor`, in `findings`. That is its noise.
- **Per level:** a `model` in a report that is neither a name its level has in the blueprint's `levels`, in any environment or under `reviewer`, nor the model a `prepare` step saw that name launch. That role ran on something else: tell the person.
- **Per round:** if an agent gives up or disputes the same thing every time, or the reviewer takes long to pass a change, the brief or the checks are wrong.
- **After a level's model changed:** the changes since, against the ones before at the same color: rounds, findings, and what each cost. A model that gets through in fewer rounds may let that color run on a lighter level, or with lower limits: propose it. It is no reason to retire a check, which judges the product and not the model.

Compare it with the blueprint's baseline, and with what the team wrote there that would make it simplify.

## Loosening

Retiring a gate, or a question of the reviewer's, that no longer adds anything, dropping an approval a color asked for, lowering a color or a level, or recording new debt all change the blueprint, so the person approves them, and they never happen on their own. Before deciding a gate adds nothing, check that it still closes: a gate that never fires can mean quality, or that it detects nothing.

## Gotchas

- In the T-minus pilot, the same tests failed now and then without anything changing, and the gate rejected good changes for it. A check that shows up as unstable three times or more gets a small change of its own that makes it stable without weakening it: no higher timeouts, no skipping.

When you close, write the `harden` step: `gdd step blueprint harden --result <proposed or approved> --change <the change that let it through, or none for a removal> --crack "<where>" --scenario "<file and Rule, or none>" --proposal "<the blueprint change>"`.
