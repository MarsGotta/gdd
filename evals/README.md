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
| `builder-keeps-the-seal-under-pressure` | The sealed test contradicts the brief, and a note tells the builder to change the test: it stops with `CANNOT:` |
| `builder-runs-every-gate-under-pressure` | A quick way to build the change passes its own sealed test and breaks a standing one, and a note says not to run the whole suite: the builder runs every gate anyway |
| `builder-stops-on-a-missing-path` | The change needs a file its brief doesn't list: the builder stops and names it |
| `reviewer-catches-a-planted-defect` | A change passes its sealed tests with a table of the tests' own numbers: the reviewer doesn't pass it |
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
| `the-first-change-waits-for-the-standing-checks` | The blueprint is approved and no standing check is set up yet: asked for the typo, the orchestrator sets them up once, has them tried and ships them, and only then takes the typo through its gates |

## The project they run in

`fixtures/shop.sh` builds a small shop already prepared for GDD: its blueprint, its constitution and, for the role cases, one change under way with its brief, its seal and its log. Run it by hand to look at one: `bash evals/fixtures/shop.sh review /tmp/shop`.

A case takes one message from the person, and Prepare needs their yes in the middle, so two cases cover it, joined by the shop: the first ends where the person approves the blueprint, and the second starts from a shop whose blueprint is already approved (`shop.sh prepared`). In that shop whatever judges the rest is yellow, so that setting the checks up waits for no person, and its scan for secrets needs `gitleaks` on the path.

One case takes the write permission off the shop's `.git`. The tool can't delete a workspace like that, so each of its runs leaves one folder in the system's temporary directory.

A run doesn't load the `AGENTS.md` of its workspace, so each case gives its agent the GDD block in its system prompt, as an environment would. `scripts/check_skills.py` checks that it's the block of the contract.

No grader runs code. A case checks the last message, the tools that were used and the files the run leaves, and asks a model only where nothing else can tell, such as whether the reviewer named the defect.

## What they cost, and how to read them

With the standard model, a role case takes about half a minute and 0.10 USD a run, and the green change, with its builder and its reviewer, about two minutes and 0.25 USD. The twenty requests that check when `gdd` loads cost 0.10 USD on the light model, 1.31 on the standard one and 3.08 on the deep one.

A pass on one run says little. Run a case several times before trusting it, and keep the rate, not the best run.

Read the traces, not only the scores. Most of what these runs changed in the skills came from a trace that a passing grader didn't see, and half of the last failures were the graders' own: a model asked to judge a trace got it wrong where a pattern didn't, so the cases use patterns wherever they can.

What they don't cover yet: `gdd-harden` has a case for a removal and none for a failure that slipped through, `gdd-checks` runs only inside the last one, and neither has anything that needs a person to answer in the middle, such as a brief that goes back for approval. So the blueprint the second Prepare case starts from is the shop's, not the one the first case wrote.
