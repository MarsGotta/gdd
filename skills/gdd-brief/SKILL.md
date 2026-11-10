---
name: gdd-brief
description: Fixes what has to pass for one change in a Gate-Driven Development (GDD) project before anyone builds it, with a first estimate of its risk, and gets the person's approval when the blueprint asks for it. Loaded by the GDD orchestrator for every change, from a typo to a payment flow, and again when an agent stops because the change doesn't fit its brief. Not for setting up a project, writing checks or building.
license: MIT
metadata:
  version: "0.1.0"
  contract: gdd/1
---

# Briefing a change

Gates come first. If you decide what to check once the result is in sight, you end up checking what the agent did and not what was needed, so everything here happens before anyone builds. You work at the standard level.

## The color: git sets the floor, and judgment can only raise it

1. **The floor.** List every path the change will touch. Each one falls in the zone whose pathspec it matches (`git ls-files -- <pathspec>` lists the existing files of a zone; a new file falls where its path matches the pattern), and the change takes the highest color among them. A path no zone covers counts as red until the person classifies it: stop, and propose updating the blueprint with `gdd-prepare`.
2. **Judgment.** Look for what the paths miss: code that a delicate zone imports or that feeds it, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files. Judgment may raise the color, never lower it, and you write down its reason. By default, a change that adds behavior someone would see is at least yellow, wherever its paths fall. Behind a switch that is off, nobody sees it yet.

On green there's no brief file, unless the blueprint's green asks for `brief-approval`: report the color with its paths and its reason, and the change goes on with the existing checks. On yellow or red, write the brief.

## The brief

Use `assets/brief.md` for the brief and `assets/scenarios.feature` for its scenarios, which go in `features/<capability>.feature`, each tagged with this brief and an id of its own.

- **Scenarios** describe what someone can observe, not how it's coded. A few on yellow, every one that matters on red, and the bad cases too: what must not happen.
- **What it touches.** In the T-minus pilot, agents stopped 69 times, and in two out of three a file they needed was missing from this list. Include:
  - the twins: a document and its translation, a scenario and its hidden counterpart, a feature and its reference page;
  - the wiring: where the new piece gets registered or connected, such as routes, step definitions, a module index or an installer;
  - the fixtures and the existing tests that assert the behavior the change alters;
  - the reference documents of anything it adds, such as an error code, a setting or a command.
  Never the sealed checks or `gdd/`: those aren't the builder's. Check that each path exists, or that its folder does: a typo in one stopped a change in the pilot.
- **Scenarios it contradicts.** Search `features/` and the tests for the behavior the request changes. If a scenario already in `features/` says the opposite, the brief changes that scenario too and says so, because that changes what has to pass. In the pilot, an agent was asked to remove every old URL and forbidden to touch the scenario that still required one.
- **Gates.** The change's own scenarios, the standing gates of every zone it touches and the checks its color asks for, each with the context the blueprint gives it.
- **What connects it.** If it builds a part, say which item of the backlog it serves and what will put it in use. In the pilot, some 28 changes were merged in six hours and not one item of the roadmap got finished: they were all parts and preparations.
- **Too big.** Propose a series of changes that the person approves in one go. Each part has the color of what it touches, leaves the product working and can be undone on its own. A part that delivers nothing yet goes behind a switch, and turning the switch on is another change, at the level of what it turns on. Splitting doesn't lower the risk.

## Decisions nobody has made

While you draft, mark each one where it comes up, in the brief or in a scenario, with `[DECISION: <the question>]`. They hide in rules two people would build differently, in limits with no number, and at the edges: empty, the maximum, twice in a row, at midnight. Ask the person and write their answer into the scenario. A brief isn't done while `git grep -n "\[DECISION:" -- gdd/briefs features` finds anything.

## Approval

Give the orchestrator the blob hash of the brief and of each scenario file (`git hash-object <file>`), so the log records exactly what has to pass. Then look in the blueprint whether this color's checks include `brief-approval`. If they don't, the change goes on. If they do, show the person the brief and its scenarios themselves, not a summary of them, because an approval is only worth what was seen, and nothing is built until they approve. On red, recommend approving it as a pull request on the platform: without an engine, an agent may share the person's git identity, so a commit alone proves little.

Once its hashes are in the log, a brief is never edited. A change to it is a new version (`version` goes up by one), and it goes back to the person only if its color asks for `brief-approval` and what has to pass changes.

## When an agent stops

It comes back here with its reason. Check the reason against the repository, then write a new version: add the missing path, change the scenario it contradicts (with the person's approval, when the color asks for it), split the change or drop it. Never loosen a check so that it fits.

## Gotchas

- Your color is an estimate. `gdd-review` recomputes it on what was actually touched, and it can only go up, so a low estimate saves nothing.
- A scenario that describes code ("the function returns null") ties the builder's hands and proves little. Describe what someone sees.
- "It's just a small change" isn't a color. The size doesn't matter; what can go wrong does.
- A rule the person gives for something not asked yet goes to that item of the backlog, not into this brief. In one run, rules for a later feature got built into the current one, before anyone had asked for that feature.

## Report

End your last message with this block, filled in. `result` is the color, or `stopped`.

```gdd-report
skill: gdd-brief 0.1.0
level: <light, standard or deep>
rounds: <how many rounds it took>
result: <what it leaves, in a word or two>
caught: <what it caught that others missed, or none>
seconds: <the time, if its environment gives it, or unknown>
tokens: <the tokens, if its environment gives them, or unknown>
```
