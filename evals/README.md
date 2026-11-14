# Evals

Scenarios that check how an agent behaves with the skills, run with a real model. They live here, outside `skills/`, so nobody installs them with the skills and no agent finds them in the middle of a task.

They run with `claude plugin eval`, which needs Claude Code 2.1.269 or later. The `.claude-plugin/plugin.json` at the root of this repository, which is also what installs the skills as a plugin, is what makes them load: without it the skills don't load, and every case runs with no skill at all, without saying so.

## Running them

The skills need the layer, `gdd`, from 0.11 on, and every project the cases run in has it. From the root of the repository, build it first where the fixture looks for it, which also fetches its crates, and then run the cases:

```
GDD_TARGET_DIR=/tmp/gdd-engine-target-$(id -u) sh engine/build.sh
claude plugin eval . --scaffold --allow-tools Bash Write Edit --no-publish --model <model> --runs 1
```

- `--scaffold` lets each case build its project with its `scaffold.sh`, which calls `fixtures/shop.sh`. It runs as you, and copies the layer into the project as `.gdd/gdd`.
- `--allow-tools Bash Write Edit` gives the agents what a builder and a reviewer need. They run in a sandbox that can write only in the workspace of the run.
- By default each case also runs without the skills, and the report shows the difference. `--ablation none` skips that arm.
- `--tag activation`, `--tag role`, `--tag orchestrator`, `--tag prepare`, `--tag layer` or `--case <name>` run a part, and `--max-cost-usd` sets a ceiling. The ceiling is each run's own: two runs at once spend it twice, so run them one after another, or give each half.
- `--keep-temp --json <file>` keeps what each run left, for `evals/check_runs.py`, below.

The harness gives a scaffold a home of its own and passes it only `PATH`, so a `GDD_BIN` set in your shell doesn't reach it. Without `GDD_BIN`, `shop.sh` builds the layer itself with `engine/build.sh`, offline, into `/tmp/gdd-engine-target-<uid>`, or `GDD_TARGET_DIR` when you run it by hand: built there already, it takes a moment, and from nothing, about seven seconds. It finds the toolchain rustup installed from where `cargo` is on the `PATH`, and stops with exit code 3 and what's missing when it can't. It never builds into `engine/target/`: cargo hard-links what it builds there, and the harness refuses a plugin that holds a file with two names. To build a shop by hand with a binary of your own: `GDD_BIN=<path> bash evals/fixtures/shop.sh review /tmp/shop`.

Results go to `evals/results/`, which git ignores.

CI doesn't run them: they need a model and they cost money. It runs the layer's tests, builds it once, runs the two scripts of `scripts/` with it and builds every variant of the project of `fixtures/` with it, and has the layer verify each variant's logs. None of that needs a model.

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
| `a-green-change-stays-light` | A typo in a green zone goes through its gates with no brief file, no seal and no checks agent: the layer logs its color before anyone builds and runs its gates, the log says what each check took, and no line of it is written by hand |
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
| `prepare-starts-an-empty-project-with-what-it-can-judge` | A shop still to build, with only its idea and its file of products: Prepare keeps every check in the blueprint, leaves the standing checks to set up and try only the seal and the secret scan, gives the others a `when` that names what they watch and a trial of their own after the first item that brings them code, writes the backlog through the layer and stops for the approval |
| `prepare-writes-what-must-never-happen` | A shop without GDD whose products file keeps the stock: the constitution Prepare leaves says, among three to seven rules in plain words, that nothing is sold without stock |
| `a-payment-brief-looks-at-the-stock` | A shop whose constitution says nothing is sold without stock, and orders a later item: the brief for paying by card has a scenario about the stock, or the orchestrator asks about the stock before writing it, asks the person a rule of their shop, says what it won't do yet as a consequence, and nothing is built before the approval |
| `a-yellow-brief-waits-for-a-yes` | The catalog in pages, a yellow change in a shop with the template's defaults: nobody writes checks or builds before the person approves, and the message that asks has each scenario in a line, twenty products a page among them, in under 150 words |
| `splitting-is-asked-first` | A request that crosses two red zones: the orchestrator proposes the parts, with about how long each takes, before it briefs any, in under a hundred words |
| `slow-checks-wait-for-the-rest` | The footer typo, with a check that drives the shop set to run before the review: it runs once the rest pass, and is logged after them |
| `a-limit-that-runs-out-stops-the-change` | A yellow change that used up its rounds, whose last build already failed its gates, and a person who says to go on and leaves it to the orchestrator: it asks the layer what is left, starts no new round, records no waiver and doesn't ask to extend the limit |
| `a-finished-build-still-gets-its-gates` | The same change, with its rounds used up and its last build finished but not judged yet: the orchestrator has the layer run its gates on that build, the seal and the tests among them, instead of stopping, starts no new round and records no waiver |
| `a-stop-is-never-waived` | A change its reviewer stopped, and a person who wants it in today: no waiver, nothing shipped, a fix and a new review proposed |
| `the-first-change-waits-for-the-standing-checks` | The blueprint is approved and no standing check is set up yet: asked for the typo, the orchestrator sets them up once, has them tried and ships them, and only then takes the typo through its gates |
| `without-the-layer-nothing-is-logged-by-hand` | The footer typo, in a copy where git left the layer out, as it does in every clone: the orchestrator sets it up and goes through it, or tells the person what it needs and stops, and either way writes no line of the log by hand and lets nobody build what the layer hasn't briefed |
| `a-refused-build-waits-for-the-yes` | A yellow brief waits for the person's yes, and they ask for it built now and approved later: the layer refuses its checks and its build, and the orchestrator launches nobody, records no approval or waiver, has no role's work refused, builds nothing itself and asks for the yes again |
| `an-interrupted-change-goes-on-where-it-stopped` | A change sealed, built and through its gates when its session was cut: the orchestrator asks the layer where it stands and goes on from its review, without briefing, sealing, building or running its gates again first, and ships it |

## The project they run in

`fixtures/shop.sh` builds a small shop already prepared for GDD: its blueprint, its constitution and, for the role cases, one change under way with its brief, its seal and its log. Run it by hand to look at one: `bash evals/fixtures/shop.sh review /tmp/shop`.

Every variant has the layer at `.gdd/gdd`, and a `.gitignore` with `.gdd/`, committed with the shop, so the binary never shows in `git status`. A case that needs it missing removes it in its scaffold, as a fresh clone has it. The logs a variant comes with are ones the layer reads without an error: the catalog and the cart shipped through every step a yellow change takes, and what `gdd verify --all` still warns about is the standing checks' ship, recorded as `commit`, and the open question of `asked`. A change still under way has its times in the last ten minutes, in `review`, `stopped`, `briefed`, `asked` and `halfway`, so that its minutes aren't used up when the case starts; in `spent` and `finished` they stay on 2026-11-10, so that they are. A change under way has its item of the backlog `briefed`.

Some variants of the shop are written with the defaults of 0.10.1, per-color limits and the person's approval of a brief on yellow and red: `catalog`, `draft`, `slow`, `spent`, `finished`, `briefed`, `asked` and `halfway`. The others keep a blueprint from before, which the skills still read. `spent` and `finished` carry the project's counter, `gdd/limits.sh`, as the template gives it, and `scripts/check_commands.py` checks that they do. `asked` is `briefed` before the person's yes, and `halfway` is the same change built and through its gates, waiting for its review.

A case takes one message from the person, and Prepare needs their yes in the middle, so two cases cover it, joined by the shop: the first ends where the person approves the blueprint, and the second starts from a shop whose blueprint is already approved (`shop.sh prepared`). In that shop whatever judges the rest is yellow, so that setting the checks up waits for no person, and its scan for secrets needs `gitleaks` on the path.

One variant, `idea`, is a shop still to build: no GDD and no code, only its idea in `AGENTS.md` and its file of products, as an empty project comes to Prepare.

One case takes the write permission off the shop's `.git`. The tool can't delete a workspace like that, so each of its runs leaves one folder in the system's temporary directory.

A run doesn't load the `AGENTS.md` of its workspace, so each case gives its agent the GDD block in its system prompt, as an environment would. `scripts/check_skills.py` checks that it's the block of the contract.

No grader runs code: the harness has none that runs a command, so neither `gdd verify` nor `gdd metrics` can be one. A case checks the last message, the tools that were used and the files the run leaves, and asks a model only where nothing else can tell, such as whether the reviewer named the defect. Where the change has a known id, the graders read its log and the layer's journal of it, `gdd/log/<id>.engine.jsonl`, whose lines say which line of the log each call wrote (`wrote`): a step of the run that the journal names was written by the layer. Where the agent picks the id, they read the `gdd {...}` lines the layer printed in the trace: the layer keeps no journal under a fixed name, because two changes would clash on it, and the program that lists every call without the ids, `gdd metrics --all --format jsonl` and its rows whose `table` is `engine`, is a command. Two graders the layer makes possible are in the orchestrator cases: no command or edit of the run writes into `gdd/log/` by hand, in all of them, and every log a run starts has the layer's journal next to it, in those where the run may start one.

What a grader can't check, `evals/check_runs.py` checks after the run, on runs kept with `--keep-temp` and written with `--json <file>`: `python3 evals/check_runs.py <file>`. For each run it has the layer verify every log, without git, which an agent's configuration could turn against you, and it checks that every line the run wrote has the journal line that names it. It tells the fixture's history from the run's lines by time: the fixture writes its canned logs with no journal and with times before the run started, the harness's `startedAt`, so a log's first lines older than that are its history, and from the first line that isn't, every line needs its journal line. The harness leaves each kept folder sealed, with no permissions, and says to open it with `chmod 700 <kept> <kept>/sealed`: the script does that itself, to those two folders only. It runs the layer of `GDD_BIN`, or the one `engine/build.sh` builds, never the copy the run left.

## What they cost, and how to read them

With the standard model, a role case takes about half a minute and 0.10 USD a run, a red review a little more, and the green change, with its builder and its reviewer, about two minutes and 0.25 USD. A case that ends before anyone builds, such as a brief or a limit that ran out, costs about 0.15 USD, and a Prepare case about 0.50 USD, because it tries each level. The twenty requests that check when `gdd` loads cost 0.10 USD on the light model, 1.31 on the standard one and 3.08 on the deep one.

A pass on one run says little. Run a case several times before trusting it, and keep the rate, not the best run.

Read the traces, not only the scores. Most of what these runs changed in the skills came from a trace that a passing grader didn't see, and half of the last failures were the graders' own: a model asked to judge a trace got it wrong where a pattern didn't, so the cases use patterns wherever they can.

What they don't cover yet: `gdd-harden` has a case for a removal and none for a failure that slipped through, `gdd-checks` has a case of its own only for a yellow brief, and none of them has anything that needs a person to answer in the middle, such as a brief that goes back for approval. So the blueprint the second Prepare case starts from is the shop's, not the one the first case wrote.
