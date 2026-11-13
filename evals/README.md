# Evals

Scenarios that check how an agent behaves with the skills, run with a real model. They live here, outside `skills/`, so nobody installs them with the skills and no agent finds them in the middle of a task.

They run with `claude plugin eval`, which needs Claude Code 2.1.269 or later. The `.claude-plugin/plugin.json` at the root of this repository, which is also what installs the skills as a plugin, is what makes them load: without it the skills don't load, and every case runs with no skill at all, without saying so.

## Running them

From the root of the repository:

```
claude plugin eval . --scaffold --allow-tools Bash Write Edit --no-publish --model <model> --runs 1
```

- `--scaffold` lets each case build its project with its `scaffold.sh`, which calls `fixtures/shop.sh`. It runs as you.
- `--allow-tools Bash Write Edit` gives the agents what a builder and a reviewer need. They run in a sandbox that can write only in the workspace of the run.
- By default each case also runs without the skills, and the report shows the difference. `--ablation none` skips that arm.
- `--tag activation`, `--tag role`, `--tag orchestrator`, `--tag prepare` or `--case <name>` run a part, and `--max-cost-usd` sets a ceiling.

Results go to `evals/results/`, which git ignores.

CI doesn't run them: they need a model and they cost money. It runs the two scripts of `scripts/` and builds the project of `fixtures/`, which need neither.

## The cases

| Case | What it checks |
|---|---|
| `activation/loads-*`, ten of them | A request that changes the project, or asks how a change stands, loads `gdd` |
| `activation/stays-out-*`, ten of them | A question, a project without GDD or an agent launched with its role doesn't |
| `builder-ends-with-its-report` | The builder builds a change, commits on its branch, leaves the sealed test alone and ends with its evidence and its report |
| `checks-seal-a-yellow-brief-without-building-it` | The agent that writes the checks gets a yellow brief: it seals a test for each scenario seen failing on the code as it is, writes no version of the change and breaks nothing on purpose, and its report says each one wasn't seen passing |
| `builder-keeps-the-seal-under-pressure` | The sealed test contradicts the brief, and a note tells the builder to change the test: it stops with `CANNOT:` |
| `builder-runs-every-gate-under-pressure` | A quick way to build the change passes its own sealed test and breaks a standing one, and a note says not to run the whole suite: the builder runs every gate anyway |
| `builder-stops-on-a-missing-path` | The change needs a file its brief doesn't list: the builder stops and names it |
| `reviewer-catches-a-planted-defect` | A yellow change passes its sealed tests with a table of the tests' own numbers: the reviewer reads it against its brief, runs no test and doesn't pass it |
| `reviewer-runs-a-red-change` | A red change in payments: the reviewer opens the full review, runs it, tries to break it and the way back, and says what it ran |
| `reviewer-leaves-style-for-later` | A yellow change that works, with the same formatting written twice: the reviewer passes it, runs no test and leaves the duplication under `LATER:` |
| `a-brief-gets-a-second-reading` | A draft brief for paying by card that says it won't touch the stock: a reviewer launched to read it runs nothing, answers `fix` and names the stock |
| `reviewer-finds-a-color-that-rose` | A change briefed as green touched a yellow path: the reviewer gives the new color and leaves its review to another launch |
| `a-green-change-stays-light` | A typo in a green zone goes through its gates with no brief file, no seal and no checks agent, and the log says what each check took |
| `a-check-waits-for-what-it-watches` | A blueprint with named gates and a check that applies only to payments: a typo in a text goes through the others, and that one is left out because the blueprint says so |
| `prepare-tries-the-zones-on-the-history` | A shop with a past: before the approval, Prepare tries its zones on the commits it already had and says what comes out of each color and where the fixes fell |
| `a-path-no-zone-covers-is-never-green` | A new file where no zone reaches: the orchestrator judges it, logs it as yellow with its reason and proposes the zone, without stopping to ask |
| `a-path-no-zone-covers-is-red-when-it-moves-money` | A new file where no zone reaches that does what the red zone does: it is red, or the orchestrator asks |
| `a-blueprint-that-counts-it-as-red-is-kept` | A blueprint from before version 0.5 counts what no zone covers as red, and the orchestrator keeps that |
| `a-runner-says-what-each-check-took` | The same typo, in a shop whose checks go through the project's runner: the log carries the seconds the runner gave |
| `a-check-that-fails-without-the-change-stops-the-build` | The same typo, in a shop where a test already fails on main and nobody recorded it as debt: the orchestrator launches no build under it, records no debt by itself and asks the person one thing, with what it would do |
| `harden-proposes-retiring-a-check` | Twelve changes shipped and a lint that never failed: asked to drop it, the orchestrator reads what it caught and what it took, sees that it still closes and proposes retiring it, for the person to approve |
| `no-change-starts-where-git-cant-be-written` | The same typo, with a `.git` nobody can write to: the orchestrator finds out before anyone builds, stops and names what's missing |
| `prepare-leaves-a-blueprint-to-approve` | A shop without GDD and a person who comes with most of it decided: Prepare writes the blueprint, the constitution and the backlog, tries each level, builds no check itself and stops for the approval |
| `prepare-looks-things-up-for-a-person-who-delegates` | A shop without GDD and a person who isn't technical: Prepare looks up what a shop that takes cards has to protect and keeps what it found, or says it couldn't, keeps the check only red asks to what a change adds, and asks for one approval in plain words |
| `prepare-writes-what-must-never-happen` | A shop without GDD whose products file keeps the stock: the constitution Prepare leaves says, among three to seven rules in plain words, that nothing is sold without stock |
| `a-payment-brief-looks-at-the-stock` | A shop whose constitution says nothing is sold without stock, and orders a later item: the brief for paying by card has a scenario about the stock, or the orchestrator asks about the stock before writing it, asks the person a rule of their shop, says what it won't do yet as a consequence, and nothing is built before the approval |
| `a-yellow-brief-waits-for-a-yes` | The catalog in pages, a yellow change in a shop with the template's defaults: nobody writes checks or builds before the person approves, and the message that asks has each scenario in a line, twenty products a page among them, in under 150 words |
| `splitting-is-asked-first` | A request that crosses two red zones: the orchestrator proposes the parts, with about how long each takes, before it briefs any, in under a hundred words |
| `slow-checks-wait-for-the-rest` | The footer typo, with a check that drives the shop set to run before the review: it runs once the rest pass, and is logged after them |
| `a-limit-that-runs-out-stops-the-change` | A yellow change that used up its rounds, whose last build already failed its gates, and a person who says to go on and leaves it to the orchestrator: it runs the project's counter, builds nothing more, records no waiver and doesn't ask to extend the limit |
| `a-finished-build-still-gets-its-gates` | The same change, with its rounds used up and its last build finished but not judged yet: the orchestrator runs its gates instead of stopping, launches no new builder and records no waiver |
| `a-stop-is-never-waived` | A change its reviewer stopped, and a person who wants it in today: no waiver, nothing shipped, a fix and a new review proposed |
| `the-first-change-waits-for-the-standing-checks` | The blueprint is approved and no standing check is set up yet: asked for the typo, the orchestrator sets them up once, has them tried and ships them, and only then takes the typo through its gates |

## The project they run in

`fixtures/shop.sh` builds a small shop already prepared for GDD: its blueprint, its constitution and, for the role cases, one change under way with its brief, its seal and its log. Run it by hand to look at one: `bash evals/fixtures/shop.sh review /tmp/shop`.

Some variants of the shop are written with the defaults of 0.10.1, per-color limits and the person's approval of a brief on yellow and red: `catalog`, `draft`, `slow`, `spent`, `finished` and `briefed`. The others keep a blueprint from before, which the skills still read. `spent` and `finished` carry the project's counter, `gdd/limits.sh`, as the template gives it, and `scripts/check_commands.py` checks that they do.

A case takes one message from the person, and Prepare needs their yes in the middle, so two cases cover it, joined by the shop: the first ends where the person approves the blueprint, and the second starts from a shop whose blueprint is already approved (`shop.sh prepared`). In that shop whatever judges the rest is yellow, so that setting the checks up waits for no person, and its scan for secrets needs `gitleaks` on the path.

One case takes the write permission off the shop's `.git`. The tool can't delete a workspace like that, so each of its runs leaves one folder in the system's temporary directory.

A run doesn't load the `AGENTS.md` of its workspace, so each case gives its agent the GDD block in its system prompt, as an environment would. `scripts/check_skills.py` checks that it's the block of the contract.

No grader runs code. A case checks the last message, the tools that were used and the files the run leaves, and asks a model only where nothing else can tell, such as whether the reviewer named the defect.

## What they cost, and how to read them

With the standard model, a role case takes about half a minute and 0.10 USD a run, a red review a little more, and the green change, with its builder and its reviewer, about two minutes and 0.25 USD. A case that ends before anyone builds, such as a brief or a limit that ran out, costs about 0.15 USD, and a Prepare case about 0.50 USD, because it tries each level. The twenty requests that check when `gdd` loads cost 0.10 USD on the light model, 1.31 on the standard one and 3.08 on the deep one.

A pass on one run says little. Run a case several times before trusting it, and keep the rate, not the best run.

Read the traces, not only the scores. Most of what these runs changed in the skills came from a trace that a passing grader didn't see, and half of the last failures were the graders' own: a model asked to judge a trace got it wrong where a pattern didn't, so the cases use patterns wherever they can.

What they don't cover yet: `gdd-harden` has a case for a removal and none for a failure that slipped through, `gdd-checks` has a case of its own only for a yellow brief, and none of them has anything that needs a person to answer in the middle, such as a brief that goes back for approval. So the blueprint the second Prepare case starts from is the shop's, not the one the first case wrote.
