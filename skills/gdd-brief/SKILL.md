---
name: gdd-brief
description: Fixes what has to pass for one change in a Gate-Driven Development (GDD) project before anyone builds it, with a first estimate of its risk. Loaded by the GDD orchestrator for every change, and when one comes back because it doesn't fit its brief or its color rose. Not for setting up a project, writing checks or building.
license: MIT
metadata:
  version: "0.7.0"
  contract: gdd/1
---

# Briefing a change

Gates come first. If you decide what to check once the result is in sight, you end up checking what the agent did and not what was needed, so everything here happens before anyone builds. You do it yourself, as the orchestrator, in your conversation with the person.

## The color: git sets the floor, and judgment can only raise it

Note the base first, the commit of the main branch the change starts from (`git rev-parse <main branch>`): the log's `brief` step carries it.

1. **The floor.** List every path the change will touch. Each one falls in the zone whose pathspec it matches (`git ls-files -- <pathspec>` lists the existing files of a zone; a new file falls where its path matches the pattern), and the change takes the highest color among them. A path no zone covers is never green, because green is only what the person marked as green: its floor is yellow, unless the blueprint says what it counts as.
2. **Judgment.** Look for what the paths miss: code that a delicate zone imports or that feeds it, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files. Judgment may raise the color, never lower it, and you write down its reason. By default, a change that adds behavior someone would see is at least yellow, wherever its paths fall. Correcting a text adds none, and behind a switch that is off nobody sees it yet. What can't be undone is never green: deleting data, a migration with no way back, anything sent outside.
3. **What no zone covers.** Judge it against what the blueprint marks as delicate, and why: red if it does what a red zone does, or feeds one, and yellow if it doesn't. If you can't tell, ask the person before going on. Write the path, its color and the reason in the `brief` step. Then add an item to the backlog that proposes its pathspec or its zone: add it yourself, now, without asking. Don't wait for an answer to it: go on with the change, and tell the person about it when you report. The blueprint changes only when they approve it, with `gdd-prepare`, and until then every change that touches the path is judged again.

The color is an estimate. `gdd-review` recomputes it on what was actually touched, and it can only go up, so a low one saves nothing. "It's just a small change" isn't a color either: the size doesn't matter; what can go wrong does.

On green there's no brief file, unless the blueprint's green asks for `brief-approval`: write the `brief` step to the log, with the color, its paths and its reason, and the change goes on with the existing checks. On yellow or red, write the brief.

## The brief

Use `assets/brief.md` for the brief and `assets/scenarios.feature` for its scenarios, which go in `features/<capability>.feature`, each tagged with this brief and an id of its own. On yellow, four sections of the brief are enough: the request, the scenarios, what it touches and how it's undone. Red fills them all.

- **Scenarios** describe what someone can observe, not how it's coded: "the function returns null" ties the builder's hands and proves little. A few on yellow, every one that matters on red, and the bad cases too: what must not happen.
- **What it touches.** In the T-minus pilot, builders stopped 72 times, and in more than half of them a file they needed was missing from this list. Include:
  - the twins: a document and its translation, a feature and its reference page;
  - the wiring: where the new piece gets registered or connected, such as routes, step definitions, a module index or an installer;
  - the fixtures and the existing tests that assert the behavior the change alters;
  - the reference documents of anything it adds, such as an error code, a setting or a command.
  Never the sealed checks or `gdd/`: those aren't the builder's. Check that each path exists, or that its folder does: a typo in one stopped a change in the pilot. If you can launch agents, give this search to one that only reads and keep its list: the files it opens don't belong in your context.
- **Scenarios it contradicts.** Search `features/` and the tests for the behavior the request changes. If a scenario already in `features/` says the opposite, the brief changes that scenario too and says so, because that changes what has to pass. In the pilot, an agent was asked to remove every old URL and forbidden to touch the scenario that still required one.
- **Gates.** The change's own scenarios, the standing gates of every zone it touches and the checks its color asks for, each with the context the blueprint gives it.
- **What connects it.** If it builds a part, say which item of the backlog it serves and what will put it in use, and set that item to `briefed`, with this brief's id, in your copy of the backlog. In the pilot, some 28 changes were merged in six hours and not one item of the roadmap got finished: they were all parts and preparations.
- **Too big.** Propose a series of changes that the person approves in one go, each with its own brief and its own approval in the log. Each part has the color of what it touches, leaves the product working and can be undone on its own. A part that delivers nothing yet goes behind a switch, and turning the switch on is another change, at the level of what it turns on. Splitting doesn't lower the risk.

## Decisions nobody has made

While you draft, mark each one where it comes up, in the brief or in a scenario, with `[DECISION: <the question>]`. They hide in rules two people would build differently, in limits with no number, and at the edges: empty, the maximum, twice in a row, at midnight. Ask the person and write their answer into the scenario. A brief isn't done while `git grep --untracked -n "\[DECISION:" -- gdd/briefs features` finds anything.

A rule the person gives for something not asked yet goes to that item of the backlog, not into this brief. In one run, rules for a later feature got built into the current one, before anyone had asked for that feature.

## Sealing and approval

Commit the brief and its scenarios on `gdd-seal/<id>`, a branch cut from the base, and write to the log that commit and the blob hash of each file (`git hash-object <file>`), so it records exactly what has to pass. Then look in the blueprint whether this color's checks include `brief-approval`. If they don't, the change goes on. If they do, show the person the brief and its scenarios themselves, not a summary of them, because an approval is only worth what was seen, and nothing is built until they approve. Recommend approving it as a pull request on the platform, above all on red: one from `gdd-seal/<id>`, which the person approves and nobody merges, because the brief reaches the main branch with the change. Where there's no platform, their own commit or their yes in this conversation is what there is.

Once its hashes are in the log, a brief changes only as a new version: the same file with `version` raised by one, in a new commit on `gdd-seal/<id>`, with that commit and its new hashes in the log. It goes back to the person only if its color asks for `brief-approval` and what has to pass changes.

## When a change comes back

- **An agent stopped.** Check its reason against the repository, then write a new version: add the missing path, change the scenario it contradicts (with the person's approval, when the color asks for it), split the change or drop it. Never loosen a check so that it fits.
- **Its color rose.** Keep its id and its base. Write the brief of the new color, or raise the one it had, from the request and the blueprint, as if nothing had been built: don't read what the builder did. With its code in sight you'd write what it did, not what was needed.

The `brief` step goes to the log when the brief is sealed, before you ask for any approval, or as soon as the color is known when there's no brief. Its `result` is the color, or `stopped`. A new version keeps the base of the first.
