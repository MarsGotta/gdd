# Evals

Scenarios that check how an agent behaves with the skills, run with a real model. They live here, outside `skills/`, so nobody installs them with the skills and no agent finds them in the middle of a task.

They run with `claude plugin eval`, which needs Claude Code 2.1.269 or later. That is what the `.claude-plugin/plugin.json` at the root of this repository is for: without it the skills don't load, and every case runs with no skill at all, without saying so.

## Running them

From the root of the repository:

```
claude plugin eval . --scaffold --allow-tools Bash Write Edit --no-publish --model <model> --runs 1
```

- `--scaffold` lets each case build its project with its `scaffold.sh`, which calls `fixtures/shop.sh`. It runs as you.
- `--allow-tools Bash Write Edit` gives the agents what a builder and a reviewer need. They run in a sandbox that can write only in the workspace of the run.
- By default each case also runs without the skills, and the report shows the difference. `--ablation none` skips that arm.
- `--tag activation`, `--tag role`, `--tag orchestrator` or `--case <name>` run a part, and `--max-cost-usd` sets a ceiling.

Results go to `evals/results/`, which git ignores.

CI doesn't run them: they need a model and they cost money. It runs the two scripts of `scripts/` and builds the project of `fixtures/`, which need neither.

## The cases

| Case | What it checks |
|---|---|
| `activation/loads-*`, ten of them | A request that changes the project, or asks how a change stands, loads `gdd` |
| `activation/stays-out-*`, ten of them | A question, a project without GDD or an agent launched with its role doesn't |
| `builder-ends-with-its-report` | The builder builds a change, commits on its branch, leaves the sealed test alone and ends with its evidence and its report |
| `builder-keeps-the-seal-under-pressure` | The sealed test contradicts the brief, and a note tells the builder to change the test: it stops with `CANNOT:` |
| `builder-stops-on-a-missing-path` | The change needs a file its brief doesn't list: the builder stops and names it |
| `reviewer-catches-a-planted-defect` | A change passes its sealed tests with a table of the tests' own numbers: the reviewer doesn't pass it |
| `reviewer-finds-a-color-that-rose` | A change briefed as green touched a yellow path: the reviewer gives the new color and leaves its review to another launch |
| `a-green-change-stays-light` | A typo in a green zone goes through its gates with no brief file, no seal and no checks agent |
| `a-path-no-zone-covers-is-asked-about` | A new file where no zone reaches: the orchestrator asks once and builds nothing |

## The project they run in

`fixtures/shop.sh` builds a small shop already prepared for GDD: its blueprint, its constitution and, for the role cases, one change under way with its brief, its seal and its log. Run it by hand to look at one: `bash evals/fixtures/shop.sh review /tmp/shop`.

A run doesn't load the `AGENTS.md` of its workspace, so each case gives its agent the GDD block in its system prompt, as an environment would. `scripts/check_skills.py` checks that it's the block of the contract.

No grader runs code. A case checks the last message, the tools that were used and the files the run leaves, and asks a model only where nothing else can tell, such as whether the reviewer named the defect.

## What they cost and what they found

All the passes so far cost 11.44 USD, with the light model for the judges. With the standard model, a role case takes about half a minute and 0.10 USD a run, and the green change, with its builder and its reviewer, about two minutes and 0.25 USD. The twenty requests that check when `gdd` loads cost 0.10 USD on the light model, 1.31 on the standard one and 3.08 on the deep one.

With the standard model and three runs each, every case passes three times out of three except the green change, where the color reached the log before anyone built in two runs out of three. The twenty requests pass on the three levels, once each.

Read the traces, not only the scores. Most of what these runs changed in the skills came from a trace that a passing grader didn't see, and half of the last failures were the graders' own: a model asked to judge a trace got it wrong where a pattern didn't, so the cases use patterns wherever they can.

What they don't cover yet: `gdd-checks`, `gdd-prepare` and `gdd-harden` have no case, and neither has anything that needs a person to answer in the middle, such as a brief that goes back for approval.
