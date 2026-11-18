---
name: gdd-brief
description: Fixes what has to pass for one change in a Gate-Driven Development (GDD) project before anyone builds it, with a first estimate of its risk. Loaded by the GDD orchestrator for every change, and when one comes back because it doesn't fit its brief or its color rose. Not for setting up a project, writing checks or building.
license: MIT
metadata:
  version: "0.13.1"
  contract: gdd/1
---

# Briefing a change

Gates come first. If you decide what to check once the result is in sight, you end up checking what the agent did and not what was needed, so everything here happens before anyone builds. You do it yourself, as the orchestrator, in your conversation with the person. The brief of a red change needs the deep level, as `gdd` says.

## The color: git sets the floor, and judgment can only raise it

1. **The floor.** List every path the change will touch, new files too: `gdd color --paths <path>...` gives the zone each one falls in, and the change takes the highest color among them. A path no zone covers is never green, because green is only what the person marked as green: its floor is yellow, unless the blueprint says what it counts as.
2. **Judgment.** Look for what the paths miss: code that a delicate zone imports or that feeds it, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files. Judgment may raise the color, never lower it, and you give its reason to `gdd brief` with `--color <color> --why "<reason>"`. By default, a change that adds behavior someone would see is at least yellow, wherever its paths fall. Correcting a text adds none, and behind a switch that is off nobody sees it yet. What can't be undone is never green: deleting data, a migration with no way back, anything sent outside.
3. **What no zone covers.** Judge it against what the blueprint marks as delicate, and why: red if it does what a red zone does, or feeds one, and yellow if it doesn't, and ask only when you can't tell which. The request was the yes: brief it at that color, its reason in `gdd brief` as above, and add the pathspec or zone you'd propose to the backlog with `gdd backlog add`, without asking. Tell the person in one line what you judged, "Nothing covers docs/ yet: I treated the page as yellow and proposed a zone for documents", and go on. The blueprint changes only when they approve it, with `gdd-prepare`, and until then every change that touches the path is judged again.

The color is an estimate. `gdd-review` recomputes it on what was actually touched, and it can only go up, so a low one saves nothing. "It's just a small change" isn't a color either: the size doesn't matter; what can go wrong does.

On green there's no brief file, unless the blueprint's green asks for `brief-approval`: run `gdd brief <id> --paths <path>... --why "<reason>" --request "<their words>"`, which writes the `brief` step with them, and the change goes on with the existing checks. On yellow or red, write the brief.

## Before any scenario

On yellow and red, answer these seven questions before you write the change's use cases as scenarios, from the request, the constitution, the blueprint, the backlog and the code. The paths say where a change lands, not what it does to the rest of the product: in one run, paying for a cart took nothing out of stock, because its brief started from the payment's paths and the stock was another item's.

1. **The product's rules.** Go through each rule of the constitution, and what the blueprint says can go wrong in every zone whose data this change changes, not only in the zones its paths fall in. Say whether this change can break each one. Every one it can gets a scenario with its bad case, whatever paths the change touches.
2. **What it leaves behind.** For each thing it creates, uses up or ends, and each value it changes that another zone owns, name in the scenario what someone sees afterwards and from where else in the product. In a shop, after payment, how many units does the catalog offer? Propose the timing under question 5.
3. **What came before.** Which features already built it reads or changes. Open their briefs and what they left for later, and say which of that this change is: a brief that said another item would complete it may mean this one.
4. **Twice, or from two places.** What happens if it happens twice in a row, or from two places at once: two tabs, two people, a retry after a timeout.
5. **What the person would answer.** A rounding, a maximum, when stock goes down: the rule is theirs. Propose it in the scenario, even if they delegated the build. "I'd take it out of stock when the payment is approved." Where the color asks for brief approval, that same yes covers the proposal. What they leave to you, or what the color has no question left for, follows their sector's usual rule. Tell them the assumption so they can correct it.
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
  Never the sealed checks or `gdd/`: those aren't the builder's. Check that each path, or its folder, exists: a typo in one stopped a change in the pilot. If you can launch agents, give this search to one that only reads and keep its list: the files it opens don't belong in your context.
- **Scenarios it contradicts.** Search `features/` and the tests for the behavior the request changes. If a scenario already in `features/` says the opposite, the brief changes that scenario too and says so, because that changes what has to pass. In the pilot, an agent was asked to remove every old URL and forbidden to touch the scenario that still required one.
- **Gates.** The change's own scenarios, the standing gates of every zone it touches and the checks its color asks for, each with the context the blueprint gives it.
- **What connects it.** If it builds a part, say which item of the backlog it serves and what will put it in use, and set that item to `briefed` with `gdd backlog briefed <item> --brief <id>`. In the pilot, some 28 changes were merged in six hours and not one item of the roadmap got finished: they were all parts and preparations.
- **One request, one change.** Don't split a request yourself, however big it is or however many red zones it crosses: every part pays its whole way through the gates. In one run, paying by card took seven changes and 33 of 70 agents. Split it only when the person asks, and propose the parts before you brief any (`--of decision`): a line for each, with what it gives them and about how long it takes, from earlier logs or said to be a guess, and one question a yes answers. The first part has to show them something: in one run, the first third of a request took 84 minutes and left the person nothing new to see. Once they say yes, each part gets its own brief and its own approval. Each part has the color of what it touches, leaves the product working and can be undone on its own. A part that delivers nothing yet goes behind a switch, and turning it on is another change, at the level of what it turns on. Splitting doesn't lower the risk.

## Decisions nobody has made

Mark open rules while drafting with `[DECISION: <the question>]`. Look for limits with no number, and edges such as empty, twice or midnight. Replace every marker with an outcome before sealing. Put the most consequential proposals to the person: none on green, at most one on yellow or two on red. With `brief-approval`, include them in its single yes. Otherwise ask once after logging the brief, only if needed. For the rest, state the assumptions as in question 5. Correct the scenario if they disagree. Ask before briefing (`--of decision`) only when the answer changes which change to brief: they requested parts, or something doesn't fit. The reference on writing to the person explains delegation. A finished brief has no markers: `git grep --untracked -n "\[DECISION:" -- gdd/briefs features`.

A rule the person gives for something not asked yet goes to that item of the backlog, not into this brief. In one run, rules for a later feature got built into the current one, before anyone had asked for that feature.

## Sealing and approval

Always in this order: write, `gdd seal`, `gdd brief`, the reading where the color has one, and only then the approval, because a person approves only what is sealed: in one run, a payment brief went to the person unsealed and unlogged.

Write the brief and its scenarios in your own copy, the one the person has open, at the paths they will have in the project, and seal them from there with `gdd seal <id> <files>`, which commits them on `gdd-seal/<id>` without touching your copy. A file the person handed over that git doesn't track goes in the seal too: in one run, the roles lacked a test provider for two rounds. Then `gdd brief <id> <files> --paths <path>...` writes the `brief` step with that commit and each file's hash: exactly what has to pass. The seal is what judges, and their copy is what they read: in one run, the scenarios lived only on the seal, and the person saw them only by asking to have them pasted in.

Have the sealed brief read before approval on red and on yellow when it changes another zone's data. To choose findings' destinations, launch a reading without `--run`. An automatic run records every finding as `went: log`. Record your choices with `gdd report <id> reading <file> --went f1=brief,f2=person`. Accept `pass` when nothing blocks. Resolve every `MISSING:` line before approval. Take up to three separate additions tied to a constitution or zone rule or loss of money or data. Seal the corrections in a new version. Where approval is required, raise disputed additions there. Other observations stay in the log.

If this color omits `brief-approval`, tell the person in one line what the change will do and go on. Otherwise follow the step the layer printed. Ask only when approval is due. Use one message under 150 words in their language:

    <What the change will do, in words of their product.>
    <Each scenario in one line, in their words.>
    <Technical decisions marked as yours, and the selected product proposals.>
    <On red: the rules of the product it touches, and up to three things it won't do yet, each as a consequence.>
    <Any disputed addition that needs their decision, and why.>
    <One question a yes answers.>

"The catalog in pages. Each page shows twenty products, in today's order. A page past the last shows the first. I decided: the links to the other pages go under the list. Do you approve it?" Before sending, count its words, check that every scenario has its line, name the file of its scenarios in their copy, which they can open as it is, and run `gdd ask <id> --of brief "<the question, in a line>"`: in one run, a yellow brief nobody approved lost its scenario of twenty products a page, which the person had caught each time they approved the brief. On red, show its scenarios beside the message as they are, because an approval is only worth what was seen. Recommend approving it as a pull request on the platform, above all on red: one from `gdd-seal/<id>`, which the person approves and nobody merges, because the brief reaches the main branch with the change. Without one, their commit or their yes here is what there is.

Once its hashes are in the log, a brief changes only as a new version: the same file with `version` raised by one, sealed and logged the same way, with `gdd seal` and `gdd brief`. It goes back to the person only if its color asks for `brief-approval` and what has to pass changes. A waiver or a change to the list of paths keeps the same approval when the scenarios stand. If the person edits their copy, that's a correction: compare it with the seal and make a new version. Never commit them on the main branch yourself: when the change goes in, undo your copy of them first (`git restore` a file the main branch has, remove a new one), so the main branch brings its own.

## When a change comes back

- **An agent stopped.** Check its reason against the repository, then write a new version: add the missing path, change the scenario it contradicts (with the person's approval, when the color asks for it), or drop the change. Never loosen a check so that it fits.
- **Its color rose.** Keep its id. Write the brief of the new color, or raise the one it had, from the request and the blueprint, as if nothing had been built: don't read what the builder did. With its code in sight you'd write what it did, not what was needed.

A change you drop ends in a `brief` step whose `result` is `stopped`: `gdd brief <id> --stop "<why>"`.
