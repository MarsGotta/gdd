---
name: gdd-brief
description: Fixes what has to pass for one change in a Gate-Driven Development (GDD) project before anyone builds it, with a first estimate of its risk. Loaded by the GDD orchestrator for every change, and when one comes back because it doesn't fit its brief or its color rose. Not for setting up a project, writing checks or building.
license: MIT
metadata:
  version: "0.21.2"
  contract: gdd/1
---

# Briefing a change

Gates come first. If you decide what to check once the result is in sight, you end up checking what the agent did and not what was needed, so everything here happens before anyone builds. You do it yourself, as the orchestrator, with the person. The brief of a red change needs the deep level, as `gdd` says.

**One request, one change, unless it asks for several things.** When the person asks for parts, or for things they could use separately, such as two backlog items, open `references/proposing-parts.md`, in this skill's folder, and propose parts once, before any brief. Without a clear yes, brief the whole request, however large, and never ask about parts again. Unless they asked for parts, one outcome with several details is one thing: propose nothing. Never count lines, zones or scenarios to decide.

## Write the rules first

For a change with a brief, start in `features/<capability>.feature`. Read the rules that cover what the change touches. Write only rules it adds or changes. In an older file, leave untouched top-level scenarios before the first rule. Group only scenarios this change adds or changes.

Write one behavior per `Rule:` line. Say who acts, under what condition and what someone can observe. Replace a measurable adjective with a number and its unit. If the rule forbids an action, say what happens when someone tries it. If two readings fit, choose values for the first scenario that give different results.

    Rule: When you set a timer for 30 seconds or more, you see "Choose a shorter time"
      @brief:timer-limit @id:timer-1
      Scenario: Exactly 30 seconds
        Given a timer set to 10 seconds
        When you set it to 30 seconds
        Then you see "Choose a shorter time"

After the seven questions below, put each scenario under the rule it checks. Each has one `When`, concrete values and one definite outcome. Spell out what someone sees in place of "correct", "adequate" or "A or B". Cover bad cases too. A few scenarios suffice on yellow. On red cover every case that matters. Keep implementation details out of scenarios: they tie the builder's hands and prove little. The builder decides how to produce the outcome.

## The color: git sets the floor, and judgment can only raise it

1. **The floor.** List every path the change will touch, new files too: `gdd color --paths <path>...` gives the zone each one falls in, and the change takes the highest color among them. A path no zone covers is never green: its floor is yellow, unless the blueprint says what it counts as.
2. **Judgment.** Look for what the paths miss: code that a delicate zone imports or that feeds it, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files. Judgment may raise the color, never lower it, and you give its reason to `gdd brief` with `--color <color> --why "<reason>"`. By default, a change that adds behavior someone would see is at least yellow, wherever its paths fall. Correcting a text adds none, and behind a switch that is off nobody sees it yet. What can't be undone is never green: deleting data, a migration with no way back, anything sent outside.
3. **What no zone covers.** Judge it against what the blueprint marks as delicate, and why: red if it does what a red zone does, or feeds one, and yellow if it doesn't, and ask only when you can't tell which. The request was the yes: brief it at that color with `--color` and `--why`, even at the floor, and add the pathspec or zone you'd propose to the backlog with `gdd backlog add`, without asking. Tell the person in one line what you judged, never as the plan's rule, "The plan doesn't cover the guide yet. I treated the change as medium risk and proposed a place for guides in the plan", and go on. The blueprint changes only when they approve it, with `gdd-prepare`, and until then every change that touches the path is judged again.

"It's just a small change" isn't a color: what can go wrong decides it.

On green there's no brief file, unless the blueprint's green asks for `brief-approval`: run `gdd brief <id> --paths <path>... --why "<reason>" --request "<their words>"`, which writes the `brief` step with them, and the change goes on with the existing checks.

## Before any scenario

On yellow and red, answer these seven questions before you write the change's use cases as scenarios, from the request, the constitution, the blueprint, the backlog and the code. The paths say where a change lands, not what it does to the rest of the product.

1. **The product's rules.** Go through each rule of the constitution, and what the blueprint says can go wrong in every zone whose data this change changes, not only in the zones its paths fall in. Say whether this change can break each one. Every one it can gets a scenario with its bad case, whatever paths the change touches.
2. **What it leaves behind.** For each thing it creates, uses up or ends, and each value it changes that another zone owns, name in the scenario what someone sees afterwards and from where else in the product. "After a car leaves, the sign at the gate shows 12 free spaces." Propose the timing under question 5.
3. **What came before.** Which features already built it reads or changes. Open their briefs and what they left for later, and say which of that this change is: a brief that said another item would complete it may mean this one.
4. **Twice, or from two places.** What happens if it happens twice in a row, or from two places at once: two tabs, two people, a retry after a timeout.
5. **What the person would answer.** Even when they delegate the build, rounding, limits and timing belong to them. Which value did you choose? State it, including any limit you ruled out, as an `@assumed` rule. When they leave it to you or no question remains, use the usual rule in their field and show your assumption so they can correct it.
6. **What waits for later.** If a part waits for another backlog item, does the product still tell the truth about what happened and what is available? If it doesn't, that part goes in this change with its scenario. A line under question 7 isn't enough.
7. **What it won't do.** Tell the person under "What it won't do yet", three at most, each as a consequence they can weigh. "Until you can save a draft, closing it means starting again." Never as what stays as it was.

Each answer ends up in a rule, a scenario, a technical decision, a path or a line of the brief, never as a paragraph of its own.

## The brief

Use `assets/scenarios.feature`. Tag each scenario with `@brief:<id> @id:<capability>-<n>`. Both tags belong to the scenario alone.

Use `assets/brief.md`, in this skill's folder, never an older brief as the model. On yellow, six sections suffice: Request, Closing it, Rules (the feature files), What it won't do yet, Undoing it and What it touches. Red fills them all. Keep the scenario ids in the front matter.

- **What it touches.** Include:
  - the twins: a document and its translation, a feature and its reference page;
  - the wiring: where the new piece gets registered or connected, such as routes, step definitions, a module index or an installer;
  - the fixtures and the existing tests that assert the behavior the change alters;
  - the reference documents of anything it adds, such as an error code, a setting or a command.
  Never the sealed checks or `gdd/`: those aren't the builder's. Check that each path, or its folder, exists. If you can launch agents, give this search to one that only reads and keep its list: the files it opens don't belong in your context.
- **Scenarios it contradicts.** Search `features/` and the tests for the behavior the request changes. If a rule or scenario in `features/` says the opposite, change it and say so in the brief, because that changes what has to pass.
- **Gates.** The change's own scenarios, the standing gates of every zone it touches and the checks its color asks for, each with the context the blueprint gives it.
- **What connects it.** If it builds a part, say which item of the backlog it serves and what will put it in use, and set that item to `briefed` with `gdd backlog briefed <item> --brief <id>`.

## Assumptions and open questions

Use a plain `Rule:` only for a rule the person gives in their own words. Put `@assumed` above every rule you choose yourself. State it firmly, including what you leave out: "The only limit is 80 characters per line." Look for a missing number and edges such as empty, twice or midnight. Keep the tag after approval. The tag records who chose the rule. The approval in the log records what the person saw. Remove the tag only when they correct the rule or state it in their own words. Use their wording and update its scenarios. A correction changes what must pass.

Ask about the most consequential unknown you cannot settle. Use an open question: "What happens when time runs out?" None on green, at most one on yellow or two on red. One brief approval covers all your assumptions. For an unknown this change can leave for later, write `@question` above `Rule: <the open question?>` and leave it without scenarios. Tag an outside fact you haven't seen `@unverified` likewise.

Before sealing, settle each rule the change needs and check it has a scenario. Reread each rule for one behavior, who acts, under what condition, what someone observes, and a unit for each measured number. Use `git grep --untracked -nE '^[[:space:]]*@(assumed|question)[[:space:]]*$' -- features` to locate the tags. Check that each open question can wait and has no scenarios. Ask before briefing (`--of decision`) only if the answer changes what you can brief. It must settle whether to take parts, what to leave out if the request won't fit, or a rule you cannot assume. Mark all other product choices the change needs `@assumed`. The person can correct them at approval or in the line you send if the color asks for none.

A rule the person gives for something not asked yet goes to that item of the backlog, not into this brief.

## Sealing and approval

Always in this order: write, `gdd seal`, `gdd brief`, the reading where the color has one, and only then the approval, because a person approves only what is sealed.

Write the brief and its scenarios in your own copy, the one the person has open, at the paths they will have in the project, and seal them from there with `gdd seal <id> <files>`, which commits them on `gdd-seal/<id>` without touching your copy. A file the person handed over that git doesn't track goes in the seal too. Then `gdd brief <id> <files> --paths <path>...` writes the `brief` step with that commit and each file's hash: exactly what has to pass. The seal is what judges, and their copy is what they read.

Have the sealed brief read before approval on red and on yellow when it changes another zone's data. To choose findings' destinations, launch a reading without `--run`. An automatic run records every finding as `went: log`. Record your choices with `gdd report <id> reading <file> --went f1=brief,f2=person`. Accept `pass` when nothing blocks. Resolve every `MISSING:` line before approval. Take up to three separate additions, including form findings. Drop one only with the line, in what the reader could read, that answers it: `f3=dropped:<file>:<line>`. Seal the corrections in a new version. Where approval is required, raise disputed additions there. Other observations stay in the log.

If this color omits `brief-approval`, tell the person in a sentence what the change will do, including any assumption they might correct, and go on. Otherwise follow the layer's step. Ask only when approval is due, in one message in their language that covers every line it prints, under 200 words on yellow and 250 on red, or longer if they asked for more detail, written as `gdd/references/person.md` says. If it doesn't fit, say less, never in notes:

    <What the change will do for whoever uses it, in whole sentences.>
    <The rules they'll notice: theirs in their words, then yours, marked as yours.>
    <What you decided so it works, grouped by what it protects, each rule named in a few words.>
    <Each scenario without a rule above it, in their words, a sentence on its own line.>
    <Any open question, left for later.>
    <What closes it, in a sentence.>
    <On red: product rules it touches and up to three things it won't do yet, each as a consequence.>
    <Any disputed addition they need to decide, and why.>
    <Where to read the scenarios. One question a yes answers.>

Example for a photo gallery:

> You'll see "Album full" when you try to add a 21st photo. I assumed you can make as many albums as you like, and we can decide later who may change the limit.
>
> We'll know it helped if albums open faster, which the page load times will show. You can read the cases in `features/albums.feature`. Shall I build it with these rules?

Check that each added or changed rule, each assumption and each scenario without a rule is in it once, and what closes it too. When you group rules, show the files beside the message on any color. Name each feature file in the person's copy. On red, show those files beside the message, open to the scenarios as sealed, so they see what they approve. Run `gdd ask <id> --of brief "<the question, in a line>"` before sending. Recommend approving it as a pull request on the platform, above all on red: one from `gdd-seal/<id>`, which the person approves and nobody merges, because the brief reaches the main branch with the change. Without one, their commit or their yes here is what there is.

If the person edits their copy, you revise a logged brief, or a change comes back (an agent stopped or its color rose), open `references/brief-versions.md`.
