---
name: gdd-brief
description: Fixes what has to pass for one change in a Gate-Driven Development (GDD) project before anyone builds it, with a first estimate of its risk. Loaded by the GDD orchestrator for every change, and when one comes back because it doesn't fit its brief or its color rose. Not for setting up a project, writing checks or building.
license: MIT
metadata:
  version: "0.11.0"
  contract: gdd/1
---

# Briefing a change

Gates come first. If you decide what to check once the result is in sight, you end up checking what the agent did and not what was needed, so everything here happens before anyone builds. You do it yourself, as the orchestrator, in your conversation with the person. The brief of a red change needs the deep level: if you aren't on it, say so to the person first.

## The color: git sets the floor, and judgment can only raise it

1. **The floor.** List every path the change will touch, new files too: `gdd color --paths <path>...` gives the zone each one falls in, and the change takes the highest color among them. A path no zone covers is never green, because green is only what the person marked as green: its floor is yellow, unless the blueprint says what it counts as.
2. **Judgment.** Look for what the paths miss: code that a delicate zone imports or that feeds it, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files. Judgment may raise the color, never lower it, and you give its reason to `gdd brief` with `--color <color> --why "<reason>"`. By default, a change that adds behavior someone would see is at least yellow, wherever its paths fall. Correcting a text adds none, and behind a switch that is off nobody sees it yet. What can't be undone is never green: deleting data, a migration with no way back, anything sent outside.
3. **What no zone covers.** Judge it against what the blueprint marks as delicate, and why: red if it does what a red zone does, or feeds one, and yellow if it doesn't, and ask only when you can't tell which. The request was the yes: brief it at that color, its reason in `gdd brief` as above, and add the pathspec or zone you'd propose to the backlog with `gdd backlog add`, without asking. Tell the person in one line what you judged, "Nothing covers docs/ yet: I treated the page as yellow and proposed a zone for documents", and go on. The blueprint changes only when they approve it, with `gdd-prepare`, and until then every change that touches the path is judged again.

The color is an estimate. `gdd-review` recomputes it on what was actually touched, and it can only go up, so a low one saves nothing. "It's just a small change" isn't a color either: the size doesn't matter; what can go wrong does.

On green there's no brief file, unless the blueprint's green asks for `brief-approval`: run `gdd brief <id> --paths <path>... --why "<reason>" --request "<their words>"`, which writes the `brief` step with them, and the change goes on with the existing checks. On yellow or red, write the brief.

## Before any scenario

On yellow and red, answer these seven questions before you write the change's use cases as scenarios, from the request, the constitution, the blueprint, the backlog and the code. The paths say where a change lands, not what it does to the rest of the product: in one run, paying for a cart took nothing out of stock, because its brief started from the payment's paths and the stock was another item's.

1. **The product's rules.** Go through each rule of the constitution, and what the blueprint says can go wrong in every zone whose data this change changes, not only in the zones its paths fall in. Say whether this change can break each one. Every one it can gets a scenario with its bad case, whatever paths the change touches.
2. **What it leaves behind.** For each thing it creates, uses up or ends (an order, a unit of stock, a booking, a session), its state right after, in a scenario that looks at it from another part of the product: the stock after a payment, the free places after a booking.
3. **What came before.** Which features already built it reads or changes. Open their briefs and what they left for later, and say which of that this change is: a brief that said another item would complete it may mean this one.
4. **Twice, or from two places.** What happens if it happens twice in a row, or from two places at once: two tabs, two people, a retry after a timeout.
5. **What the person would answer.** Which rule here the person would have an answer for: a rounding, a maximum, an order, when stock goes down, a case that is free or empty. That rule is theirs, even if they left earlier decisions to you: mark it as a decision and ask it as your proposal, "I'd take it out of stock when the payment is approved. Yes?", because "you decide" covers how it gets built, not a rule of their product. If they leave this one to you too, or the color has no question left, your proposal is the rule: what is usual in their sector, said in your last message as an assumption, "I assumed this; tell me if not."
6. **What waits for later.** If a part is left for another item of the backlog, whether the product still tells the truth meanwhile: nothing shown as paid that wasn't, nothing offered that can't be had. If it doesn't, that part goes in this change.
7. **What it won't do.** Tell the person under "What it won't do yet", three at most, each as a consequence they can weigh: "paying won't take products out of stock yet, so the shop can sell what it no longer has." Never as what stays as it was.

Each answer ends up in a scenario, a decision, a path or a line of the brief, never as a paragraph of its own.

## The brief

Use `assets/brief.md` for the brief and `assets/scenarios.feature` for its scenarios, which go in `features/<capability>.feature`, each tagged with this brief and an id of its own. They are the change's use cases, in Gherkin, and they come first: `gdd-checks` writes a test from each one, sees it fail and seals it, and only then is the change built. On yellow, five sections of the brief are enough: the request, what it won't do yet, the scenarios, what it touches and how it's undone. Red fills them all.

- **Scenarios** describe what someone can observe, not how it's coded: "the function returns null" ties the builder's hands and proves little. A few on yellow, every one that matters on red, and the bad cases too: what must not happen.
- **What it touches.** In the T-minus pilot, builders stopped 72 times, and in more than half of them a file they needed was missing from this list. Include:
  - the twins: a document and its translation, a feature and its reference page;
  - the wiring: where the new piece gets registered or connected, such as routes, step definitions, a module index or an installer;
  - the fixtures and the existing tests that assert the behavior the change alters;
  - the reference documents of anything it adds, such as an error code, a setting or a command.
  Never the sealed checks or `gdd/`: those aren't the builder's. Check that each path exists, or that its folder does: a typo in one stopped a change in the pilot. If you can launch agents, give this search to one that only reads and keep its list: the files it opens don't belong in your context.
- **Scenarios it contradicts.** Search `features/` and the tests for the behavior the request changes. If a scenario already in `features/` says the opposite, the brief changes that scenario too and says so, because that changes what has to pass. In the pilot, an agent was asked to remove every old URL and forbidden to touch the scenario that still required one.
- **Gates.** The change's own scenarios, the standing gates of every zone it touches and the checks its color asks for, each with the context the blueprint gives it.
- **What connects it.** If it builds a part, say which item of the backlog it serves and what will put it in use, and set that item to `briefed` with `gdd backlog briefed <item> --brief <id>`. In the pilot, some 28 changes were merged in six hours and not one item of the roadmap got finished: they were all parts and preparations.
- **Too big.** Split a request only when it would go over the blueprint's `lines_per_change` or cross two red zones, and propose the split to the person before you brief any part: a line for each part, with what it gives them and about how long it takes, from earlier logs or said to be a guess, and one question a yes answers. "I'd do accounts in two: first signing up and in, about 30 minutes, so customers can open one; then their past orders, about 20. Yes?" Every part pays its whole way through the gates, so the first one has to show them something: in one run, the first third of a request took 84 minutes and left the person nothing new to see. Once they say yes, each part gets its own brief and its own approval. Each part has the color of what it touches, leaves the product working and can be undone on its own. A part that delivers nothing yet goes behind a switch, and turning the switch on is another change, at the level of what it turns on. Splitting doesn't lower the risk.

## Decisions nobody has made

While you draft, mark each one where it comes up, in the brief or in a scenario, with `[DECISION: <the question>]`. They hide in rules two people would build differently, in limits with no number, and at the edges: empty, the maximum, twice in a row, at midnight. Ask the person, with what you'd propose, in one message and within what the color allows: no question on green, one on yellow, two on red, the one that weighs most first. Write their answer into the scenario. What they leave to you, and what no question was left for, is an assumption, as question 5 says, and the `gdd` skill's reference on writing to the person says what "you decide" covers. A brief isn't done while `git grep --untracked -n "\[DECISION:" -- gdd/briefs features` finds anything.

A rule the person gives for something not asked yet goes to that item of the backlog, not into this brief. In one run, rules for a later feature got built into the current one, before anyone had asked for that feature.

## Sealing and approval

Always in this order: write, `gdd seal`, `gdd brief`, the reading where the color has one, and only then the approval, because a person approves only what is sealed: in one run, a payment brief went to the person unsealed and unlogged.

Write the brief and its scenarios in your own copy, the one the person has open, at the paths they will have in the project, and seal them from there with `gdd seal <id> <files>`, which commits them on `gdd-seal/<id>` without touching your copy. Then `gdd brief <id> <files> --paths <path>...` writes the `brief` step with that commit and each file's hash: exactly what has to pass. The seal is what judges, and their copy is what they read: in one run, the scenarios lived only on the seal, and the person saw them only by asking to have them pasted in.

On red, and on yellow when the change changes data another zone owns, have it read once it is sealed, before you ask for any approval: `gdd launch <id> reading`, and when it answers, `gdd report <id> reading <file>` with where each finding went, `--went f1=brief,f2=person`, `log` or `dropped`. Take into the brief what you agree with, as a new version. What you don't, three at most, goes to the person with the approval, or stays in the log where the color asks for no `brief-approval`.

Then look in the blueprint whether this color's checks include `brief-approval`. If they don't, tell the person in one line what the change will do, and go on without waiting for them. If they do, nothing is built until they approve, and you ask in one message, under 150 words, in their language, in this shape:

    <What the change will do, in words of their product.>
    <Each scenario in one line, in their words.>
    <Each decision on how it's built, marked as yours; each rule of their product, as your proposal and its question.>
    <On red: the rules of the product it touches, and up to three things it won't do yet, each as a consequence.>
    <If it was read: what you left out of its reading, a line each, with why.>
    <One question a yes answers.>

"The catalog in pages. Each page shows twenty products, in today's order. A page past the last shows the first. I decided: the links to the other pages go under the list. Do you approve it?" Before sending, count its words, check that every scenario has its line, name the file of its scenarios in their copy, which they can open as it is, and run `gdd ask <id> --of brief "<the question, in a line>"`: in one run, a yellow brief nobody approved lost its scenario of twenty products a page, which the person had caught each time they approved the brief. On red, show its scenarios beside the message as they are, because an approval is only worth what was seen. Recommend approving it as a pull request on the platform, above all on red: one from `gdd-seal/<id>`, which the person approves and nobody merges, because the brief reaches the main branch with the change. Where there's no platform, their own commit or their yes in this conversation is what there is.

Once its hashes are in the log, a brief changes only as a new version: the same file with `version` raised by one, sealed and logged the same way, with `gdd seal` and `gdd brief`. It goes back to the person only if its color asks for `brief-approval` and what has to pass changes. If the person edits their copy, that's a correction: compare it with the seal and make a new version. Never commit them on the main branch yourself: when the change goes in, undo your copy of them first (`git restore` a file the main branch has, remove a new one), so the main branch brings its own.

## When a change comes back

- **An agent stopped.** Check its reason against the repository, then write a new version: add the missing path, change the scenario it contradicts (with the person's approval, when the color asks for it), split the change or drop it. Never loosen a check so that it fits.
- **Its color rose.** Keep its id. Write the brief of the new color, or raise the one it had, from the request and the blueprint, as if nothing had been built: don't read what the builder did. With its code in sight you'd write what it did, not what was needed.

A change you drop ends in a `brief` step whose `result` is `stopped`, through `gdd log`.
