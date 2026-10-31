# Roles

Who does what in a GDD project: the human, the Orchestrator, the Builder and the Reviewer. For
each, what it reads, what it may write, what it must not touch, which model plays it and which
commands it runs. Consult it when you need to know whether a role may read, write or run
something.

## Overview

| Role | Played by | Started by | `GDD_ROLE` | Path Guard list |
|---|---|---|---|---|
| [The human](#the-human) | A person | | unset | none |
| [The Orchestrator](#the-orchestrator) | An agent, strong model | The human (`gdd-status`, `gdd-change`...), or the recipe for a replan | `orchestrator` in a replan | none of its own |
| [The Builder, test mode](#test-mode) | An agent, by color | The recipe | `builder-test` | `all` + `builder_test` |
| [The Builder, build mode](#build-mode) | An agent, by color | The recipe | `builder-build` | `all` + `builder_build` |
| [The Reviewer](#the-reviewer) | An agent of another model family | The recipe | `reviewer` | `all` + `reviewer` |
| The machines | `gdd` and the gate commands | | | |

The machines only say yes or no, and why: they run the gates, Path Guard and the release steps.

## What an agent may run

When gdd starts an agent, it sets `GDD_ROLE` to the agent's role. With `GDD_ROLE` set, gdd runs
only what the agent's work needs:

- `gdd g2`, `gdd guard`, `gdd note`, `gdd status`, `gdd classify`, `gdd inbox`, `gdd report`
  and `gdd scope`;
- `gdd --help`, `gdd help` and `gdd --version`.

Any other command fails before it does anything, whatever the role and whatever the agent's own
permissions allow: no agent can start, prepare, merge, park or release a change, set up or adopt
the project, or write the GDD state. For example, a Builder that runs `gdd selftest` gets:

```text
error: Builders and Reviewers cannot run gdd selftest: an agent gdd started may only run gdd g2, gdd guard, gdd note, gdd status, gdd classify, gdd inbox, gdd report and gdd scope.
```

The replanning Orchestrator (`GDD_ROLE=orchestrator`) gets the same message. The message names
Builders and Reviewers, but it applies to every agent gdd started, the Orchestrator included.
The commands that
already refused a Builder or a Reviewer in their own words keep them: `gdd gate`,
`gdd changelog-line`, `gdd resume`, `gdd release` without a step, `gdd learn` and
`gdd upgrade`. For example:

```text
error: A Builder or Reviewer cannot open or close gates.
```

The screens (the Board, the sitting, the `gdd init` form) never open for an agent. A person has
no `GDD_ROLE` and runs every command. A `gdd g2` run with any `GDD_ROLE` is not logged.

## Checks around every agent call

Around every agent call of the recipe, gdd checks two things, and either one parks the change:

- No agent changed the GDD state, of its change or the shared one (`.gdd/`, links included), or
  left a `.gdd` folder in its worktree. What the gdd commands it may run write (notes, the
  paths the scope check accepted, its gate report) is allowed; anything else is put back.
- The Reviewer changed nothing: the working tree outside `.gdd/` is the same after its call,
  down to the kind of each path, its permissions and where its links point.

## The human

The stakeholder. Does not read code; decides what only a person can decide, in as few sittings
as possible.

**Reads:** the Project Brief (`docs/brief.md`), the scenarios of a change (not the hidden
examples), the red zones and their reasons, the plan of a red change, the Decision Inbox
(`.gdd/changes/<id>/inbox.md`), the demo and the G4 page (`gdd sitting`, the Board).

**Decides:**

| Stop | How |
|---|---|
| Setup answers (phase 0) and Discover (phase 1) | In conversation with the Orchestrator. |
| The sitting of a change: scenarios (G1), red zones, plan of a red change | `gdd sitting <id>`, or the Orchestrator records the answer with `gdd gate G1 ok <id>`, `gdd gate zones ok` and `gdd gate plan ok <id>`, each `--by human`. |
| G4 | `gdd resume <id> --g4 ok`, `--g4 ko` or `--g4 polish --note "..."`. |
| A parked change | Reads its Decision Inbox; one message per change. |

**Writes:** through the Orchestrator, the harness on the main branch (`gdd/*.toml`,
`.github/`, the agents' folders); `docs/standards.md`, the project's quality bar. Path Guard
does not judge the human.

**Runs:** `gdd init` (its form also runs the first install of each stack), `gdd board` and
`gdd sitting`, `gdd upgrade` (then merges the `gdd-upgrade` branch), and the agent commands
`gdd-status`, `gdd-setup`, `gdd-discover`, `gdd-change`, `gdd-learn`, `gdd-hotfix` and
`gdd-report` with the Orchestrator. Every other command is open to a person too; the
Orchestrator runs most of them for you.

Human stops per change: 0 in the Fast Lane, 0 to 2 in the Full Lane, 2 for a red change (the
sitting, then G4), 0 or 1 for a hotfix; at most one message per change.

## The Orchestrator

Holds the map: architecture, ADRs, zones, contracts, scenarios, state and log. Works with the
human (interactive), and once per stuck change without them (headless replan).

**Reads:** `gdd/project.toml`, `docs/brief.md`, `gdd status`, `gdd report`, the records
and Decision Inboxes in `.gdd/changes/`, and for a replan the package, `g2.json` and the
Builder's outputs.

**Writes:**

| What | Where | When |
|---|---|---|
| Project Brief, technology ADR, skeleton, gate commands, CI | `docs/brief.md`, `docs/adr/0001-technology.md`, `gdd/`, `.github/` | Setup, on main |
| Discover notes, ADRs | `docs/discover/YYYY-MM-DD-<topic>.md`, `docs/adr/` | Discover |
| Zone map | `gdd/zones.toml`, on main | End of Discover; approved at the next sitting |
| A change | `gdd new-change <id> ...` | Change |
| Scenarios and hidden examples | `features/`, `features/hidden/`, on the change's branch | Before G1 |
| Plan of a red change | `.gdd/changes/<id>/plan.md` | Before the sitting |
| Minimum context package | `.gdd/changes/<id>/package.md` | Before the launch |
| Decisions taken, questions | `gdd note <id> decided "..."`, `gdd note <id> needs "..."` | Any time |
| Replan | `.gdd/changes/<id>/replan.md`, ending with `NEXT: test` or `NEXT: build` | Headless, G2 still red |
| Learn proposals | `docs/learn/YYYY-MM-DD.md` | Learn |

**Must not:** write product code (outside a spike), resolve merge conflicts, loosen a gate,
edit approved scenarios, or change `gdd/` without the human. In a replan it writes only
`replan.md` of the GDD state; the Claude Code sheet also denies it `.gdd/log.jsonl` and every
`change.json`.

**Runs:** with the human, every gdd command the skills name. In a replan, `gdd status`,
`classify`, `note`, `report` and git reads: `GDD_ROLE=orchestrator` refuses the rest.

**Commands:** `gdd-status`, `gdd-setup`, `gdd-discover`, `gdd-change`, `gdd-learn`,
`gdd-hotfix`, `gdd-report` (with the human); `gdd-replan` (headless, `change=<id>
reason=g2`).

**Model:** `[orchestrator]` of `gdd/models.toml`; `gdd init` gives it the strong model of the
first agent.

## The Builder

Writes the tests and the code of a change, in two modes, always in separate calls. Works in the
change's worktree, without production credentials (see [Environment
variables](environment.md#what-agents-get)), without asking anyone: nobody answers.

Both modes:

- read the instructions of every stack of the project (`gdd-stack-<name>`) and their package,
  `<state>/.gdd/changes/<id>/package.md`;
- check their work with `gdd g2 --role <their role>`, which is what counts;
- may run only `gdd g2`, `gdd guard`, `gdd note` and git reads (`git status`, `git diff`,
  `git log`, `git show`), each command on its own; the agent sheet allows no more, and
  `GDD_ROLE` refuses every gdd command [outside the list](#what-an-agent-may-run);
- never edit approved tests or `.feature` files, touch protected paths, silence warnings or
  skip checks, write special cases for the examples, or add a dependency without noting it;
- write what they decided with `gdd note <id> decided "..."`, and what needs the human with
  `gdd note <id> needs "..."`.

**Model:** `[builder.<color>]` of `gdd/models.toml` for build mode, by the color of the change;
`[builder_test]` for test mode, every color. Cheap or local in green, more capable in red.

### Test mode

**Command:** `gdd-build-test`, with `change=<id>`, plus `mode=test` (or `mode=test
after=replan`, or `mode=test fix=review`) from the recipe. `GDD_ROLE=builder-test`.

**Reads:** the package, the `.feature` files it names, the code under the package paths (to
read). After a replan, `replan.md`; with `fix=review`, `review.md`.

**Writes:** the step definitions of the scenarios, against the domain language, with real
assertions; characterization tests before touching existing yellow or red code. The new tests
must fail because the behavior is missing.

**Must not touch:** `[protected] all` and `builder_test`: the production code (`src/**` with
`gdd init`'s sheets; also `crates/*/src/**` for Rust).

Runs when the change needs tests first, before build mode; again when a replan says
`NEXT: test`; and first in the fix round after a Reviewer's KO when a finding names a file that
only test mode may write. Path Guard runs after it: a failure parks the change.

### Build mode

**Command:** `gdd-build`, with `change=<id>` and one of `mode=build`, `mode=build fix=g3`,
`mode=build fix=review`, `mode=build fix=polish`, `mode=build after=replan`, or `mode=spike`
(and `variant=<n>`) for a spike. `GDD_ROLE=builder-build`.

**Reads:** the package; `docs/standards.md` when it exists; the last `g2.json`; with `fix=g3`,
`g3.json`; with `fix=review`, `review.md`; with `after=replan`, `replan.md`; with
`fix=polish`, the Polish notes of `inbox.md`; the code under the package paths.

**Writes:** the domain, the wiring and the unit tests, until G2 is green. A spike writes the
smallest thing that answers its question and an ADR in `docs/adr/` with a `## Decision`
section.

**Must not touch:** `[protected] all` and `builder_build`: the approved scenarios and the steps
written in test mode (`features/**` and the stack's step folder). A path outside the package
of the change's color or lower is accepted and listed; a higher one fails the scope check.

## The Reviewer

Checks the whole change after G3, from another model family than the Builder, so it does not
share its blind spots. Answers OK or KO with reasons; one fix; opens no new rounds.

**Command:** `gdd-review`, with `change=<id> round=1` or `round=2`. `GDD_ROLE=reviewer`.

**Reads:** the package, `change.json` (the color, the gates, `review` in `decision`, the
earlier reviews in `reviews`), the change's `.feature` files, the whole diff (`git diff
main...HEAD`), `g2.json`, `g3.json`, `docs/standards.md`, the stack instructions.

**Checks:** every step does what its scenario says, with real assertions; no special cases for
the examples; the change stays in its package; in red, security; work that only looks finished;
a new step that repeats an existing one. Quick depth reads the files the change touched, the
package and the scenarios; deep depth also the code around it. Each finding gets an id, a class
of bug, `file:line` and every other place of that class.

**Writes:** nothing. gdd keeps its final text as `.gdd/changes/<id>/review.md` and
`outputs/reviewer-<n>.md`. An OK ends with a `## How it works` section, shown first on the G4
page and in the Decision Inbox. The last line is `VERDICT: OK` or `VERDICT: KO`; without a
readable verdict, it counts as KO.

**Must not touch:** anything: `[protected] reviewer` is `["**"]`, it runs read-only, and gdd
parks the change when its working tree changed.

**Runs:** git reads only.

**Model:** `[reviewer]` of `gdd/models.toml`. `gdd prepare` refuses a change whose Reviewer is
of the Builder's family, unless `same_family_accepted` records the day the human accepted it.

## Default models

`gdd init` fills `gdd/models.toml` from `--agents` (the first one builds and orchestrates) and
the defaults of each agent. The Reviewer is the first other agent of another family; with one
family only, `gdd init` needs `--accept-same-family` and the Reviewer is the first agent again.

| Role | Model |
|---|---|
| `[orchestrator]` | strong, of the first agent |
| `[builder.red]` | strong, of the first agent |
| `[builder.yellow]` | standard, of the first agent |
| `[builder.green]` | light, of the first agent |
| `[builder_test]` | standard, of the first agent |
| `[reviewer]` | strong, of the Reviewer's agent, with its effort |

| Agent (`integration`) | Family | Strong | Standard | Light | Reviewer effort |
|---|---|---|---|---|---|
| `claude` (Claude Code) | `anthropic` | `opus` | `sonnet` | `haiku` | none |
| `codex` (Codex) | `openai` | `gpt-6-astra` | `gpt-6-astra` | `gpt-6-astra` | `high` |
| `opencode` (OpenCode) | `minimax` | `minimax/MiniMax-M3` | `minimax/MiniMax-M3` | `minimax/MiniMax-M3` | none |

With `--agents claude codex`: Claude Code builds and orchestrates (opus, sonnet, haiku) and
Codex reviews (`gpt-6-astra`, effort `high`).

## Agent permissions

The agent sheets give each role its arguments. No role gets Model Context Protocol (MCP) servers. Each runs in the
change's worktree and may write its own change's state folder only through `gdd`.

| Role | Claude Code | Codex | OpenCode |
|---|---|---|---|
| Builder, both modes | `--permission-mode acceptEdits`; allowed: read, edit, write, `gdd g2`, `guard`, `note`, git reads, `ls`; denied: `gdd gate`, `launch`, `resume`, `merge`, `release`, `git commit`, `switch`, `checkout`, `push`, the web, editing `.gdd/` | `--sandbox workspace-write`, plus its change's state folder | no flags yet |
| Reviewer | `--permission-mode plan`; allowed: read and git reads; the same denials | `--sandbox read-only` | no flags yet |
| Orchestrator (replan) | `--permission-mode acceptEdits`; allowed: read, edit, write, `gdd status`, `classify`, `note`, `report`, git reads; denied: editing `.gdd/log.jsonl` and any `change.json` | `--sandbox workspace-write`, plus its change's state folder | no flags yet |

gdd's own checks (the GDD state, a read-only Reviewer and the
[commands an agent may run](#what-an-agent-may-run)) apply to every agent, OpenCode included.

## See also

- [Keeping agents honest](../explanation/safety.md): why Path Guard, hidden examples and the
  state checks exist.
- [Gates](gates.md): what each role's work must pass.
- [Choose agents and models](../how-to/agents-and-models.md): change who plays each role.
- [models.toml](configuration.md#modelstoml): every key of the models file.
