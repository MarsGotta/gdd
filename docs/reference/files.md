# Files gdd writes

Every file and folder gdd writes in a project, what each one holds, which are committed, and
what the merge commit of a change carries.

## A project after gdd init

`gdd init --name notes --platform cli --stack python --agents claude codex` in an empty
repository commits these files (the agents' commands are listed once per agent folder):

```text
.agents/skills/gdd-<command>/SKILL.md   # Codex: the GDD commands and gdd-stack-python
.claude/skills/gdd-<command>/SKILL.md   # Claude Code: the same
.github/workflows/gdd.yml               # CI: the stack setup, gitleaks, gdd, gdd ci
.gitignore                              # the stack's lines and .gdd/
AGENTS.md                               # instructions for agents, when the project had none
CLAUDE.md                               # @AGENTS.md, when the project had none
docs/brief.md                           # the Project Brief
features/.gitkeep                       # the scenario folder
gdd/commands.toml
gdd/generated.toml
gdd/models.toml
gdd/project.toml
gdd/rules.toml
gdd/zones.toml
osv-scanner-licenses.toml               # from here on: the Python skeleton
osv-scanner.toml
pyproject.toml
src/notes/__init__.py
src/notes/greeting.py
tests/__init__.py
tests/acceptance/__init__.py
tests/acceptance/steps/__init__.py
tests/acceptance/test_features.py
tests/unit/__init__.py
tests/unit/test_greeting.py
```

| Path | What it is |
|---|---|
| `gdd/` | The configuration: see [Configuration](configuration.md). |
| `docs/brief.md` | The Project Brief, from `--name`, `--description`, `--platform`, `--stack` and `--sensitive`. The Orchestrator completes it at Setup. |
| `AGENTS.md`, `CLAUDE.md` | Written only when the project has none; otherwise gdd prints the line to add. |
| `.gitignore` | gdd appends the stack's ignored paths and `.gdd/`. |
| Agent folders | `.claude/skills/<name>/SKILL.md` (Claude Code), `.agents/skills/<name>/SKILL.md` (Codex), `.opencode/commands/<name>.md` (OpenCode), for the agents of `--agents`. `gdd agents install` writes them again; `gdd agents check` names any file missing or edited. |
| Skeleton | Per stack, only when the folder has no project of that stack: see [Stacks](stacks.md). |

`gdd init` never overwrites a file, never writes through a link, and commits only what it
wrote (`gdd init <version>`), in a new repository when the folder is in none. gdd writes no
`bin/` folder: people, agents and CI all run the `gdd` on the `PATH`. The [`.gdd/bin/`](#gdd)
folder listed further down is something else: a local folder for programs a project keeps out
of git, which gdd protects but never writes.

The Setup form of `gdd init` then runs the first install of each stack and commits the lock
files, as `gdd bootstrap` does after `gdd init` with flags. `gdd adopt` commits
`gdd/debt.toml`.

## Committed and local

| Path | Committed | Written by |
|---|---|---|
| `gdd/*.toml` | yes | `gdd init`, `gdd adopt`, `gdd upgrade`, a person; see [Who may change them](configuration.md#who-may-change-them) |
| `.github/workflows/gdd.yml`, agent folders, `AGENTS.md`, `CLAUDE.md` | yes | `gdd init`, `gdd agents install` |
| `docs/`, `features/`, code and tests | yes | the Orchestrator and the Builder, through changes |
| `CHANGELOG.md` | yes, when the project keeps one | `gdd changelog --write`, `gdd release` |
| `.gdd/` | no: `gdd init` adds `.gdd/` to `.gitignore` | gdd, and a few files the Orchestrator writes |

The record of each change reaches git only through its [merge commit](#the-merge-commit).

## .gdd/

The local state: what gdd knows about the project and each change. Every worktree of a
repository shares the `.gdd/` of its main checkout (the state root), so a change runs in its own
worktree and still reads and writes the same records. gdd refuses to run in a worktree of a
bare repository.

```text
.gdd/
├── log.jsonl                 # one JSON line per agent call, gate decision and machine step
├── g0.json                   # G0, opened by the machines
├── zones.json                # the human's approval of the red zones
├── selftest.json             # the last harness self-test
├── headless_sessions.txt     # sessions gdd started for its agents
├── imported_requests.json    # requests of interactive sessions already in the log
├── agents.lock  launch.lock  merge.lock  state.lock
├── adoption/<command>.txt    # gdd adopt: what each failing command said
├── learn/<tool>.txt          # gdd learn: what each Learn tool said
├── selftest/kept/            # gdd selftest: a file set aside while the check runs
├── upgrade/                  # gdd upgrade: scratch folder, removed after each run
├── bin/                      # protected; gdd does not write it
└── changes/<id>/
    ├── change.json           # the record
    ├── inbox.md              # the Decision Inbox
    ├── plan.md               # the plan of a red change (the Orchestrator)
    ├── package.md            # the minimum context package (the Orchestrator)
    ├── replan.md             # the last replan (the Orchestrator, headless)
    ├── review.md             # the last review (the Reviewer's text, kept by gdd)
    ├── g2.json  g3.json      # the last report of each gate
    ├── snapshot.json         # gdd snapshot
    ├── snapshot-build.json   # the snapshot taken after test mode
    ├── run.json              # while the change waits at G4
    ├── outputs/              # every agent call: <role>-<n>.md and <role>-<n>.raw
    └── demo/                 # what the demo step leaves for G4
```

| Path | Written by | Contents |
|---|---|---|
| `log.jsonl` | gdd | See [log.jsonl](#logjsonl). |
| `g0.json` | `gdd gate G0` | `decision`, `by` (`machines`), `ts`, `note`. |
| `zones.json` | `gdd gate zones` | `decision`, `by` (`human`), `ts`, `red_hash` (16 hex characters of the SHA-256 of the red zones), `red` (the red zones as approved, every key), `note`. |
| `selftest.json` | `gdd selftest` | `ok`, `summary` (`harness 4/4`), `detail` (each check's `test` and `ok`), `ts`. |
| `headless_sessions.txt` | gdd, after each agent call that reports a session | One session id per line, so `gdd session` leaves them out. |
| `imported_requests.json` | `gdd session` | The ids of the requests already counted, as a JSON list. A `.new` file holds the next list until the log has its lines. |
| `*.lock` | gdd | Empty lock files: `merge.lock` (one merge at a time), `launch.lock` (the cap of `[parallel] max`), `agents.lock` (held by every agent call, and alone by a person's write to shared state), `state.lock` (the turn to take it). |
| `adoption/<command>.txt` | `gdd adopt` | What the failing command's output keeps, for its Refactor. |
| `learn/<tool>.txt` | `gdd learn` | All a Learn tool said. |
| `bin/` | not gdd | Programs a project keeps out of git, such as its own build of gdd. In `[protected] all` and the harness red zone, and checked around every agent call. |
| `changes/<id>/change.json` | gdd only | See [change.json](#changejson). |
| `changes/<id>/inbox.md` | gdd, after every change of the record | See [inbox.md](#inboxmd). |
| `changes/<id>/plan.md`, `package.md` | the Orchestrator (`gdd-change`) | Free text. The Builder and the Reviewer read `package.md`. |
| `changes/<id>/replan.md` | the Orchestrator in a replan | Free text; the last line that starts with `NEXT:` says `test` or `build`. gdd removes it before each replan. |
| `changes/<id>/review.md` | gdd, from the Reviewer's final text | Ends with `VERDICT: OK` or `VERDICT: KO`. gdd removes it before each review. |
| `changes/<id>/g2.json`, `g3.json` | `gdd g2`, `gdd g3`, the recipe | See [Gate reports](#gate-reports). |
| `changes/<id>/snapshot.json` | `gdd snapshot` | Path to SHA-256 (or `deleted`) of what the change touched: `gdd guard --since-snapshot` judges only what changes after it. |
| `changes/<id>/snapshot-build.json` | the recipe, after test mode | The same, so build mode is judged only for its own work. |
| `changes/<id>/run.json` | the recipe | `{"paused_at": "G4"}` while the change waits for `gdd resume`; removed after the answer. |
| `changes/<id>/outputs/` | gdd, after every agent call | `<role>-<n>.md`: the agent's final text; `<role>-<n>.raw`: its whole output. `<role>` is `builder-test`, `builder-build`, `reviewer` or `orchestrator`; `<n>` counts the role's calls on the change, from 1. |
| `changes/<id>/demo/` | the project's `[release] demo` command | Video, screenshots or the readable run of the scenarios. |

## change.json

The record of a change: what it plans, what its gates decided, its notes and its state.
Indented JSON. Only gdd writes it. The gdd commands an agent may run (`gdd note`, a gate in
build mode) change only `inbox` and `out_of_scope`, and gdd puts back any other edit an agent
makes.

The record of a change merged and promoted in a throwaway project, trimmed:

```json
{
  "build_start": "ca7bc9c0d5369ef863aab8d6f7651f64056215ed",
  "changelog": {
    "breaking": false,
    "line": "The cart shows its total",
    "section": "Added"
  },
  "commit": "8b14c1a9ab22d8931b010ad51499c8242ff03e4a",
  "created": "2026-10-30T13:11:40+00:00",
  "decision": {
    "builder": {
      "integration": "claude",
      "model": "sonnet"
    },
    "builder_session": "separate",
    ...
    "type": "Feature"
  },
  "features": [
    "features/cart.feature"
  ],
  "gates": {
    "G1": {
      "by": "human",
      "decision": "ok",
      "hashes": {
        "features/cart.feature": "06a581fd190d4dc2e48f33c21323302a67e522e74b8bacce3dd2b51c3fdce716"
      },
      "note": "approved at the sitting",
      "ts": "2026-10-30T13:11:46+00:00"
    },
    ...
  },
  "id": "cart-total",
  "inbox": [
    {
      "kind": "decided",
      "text": "Kept the total as an integer",
      "ts": "2026-10-30T13:11:53+00:00"
    },
    ...
  ],
  "min_color": null,
  "out_of_scope": [
    "docs/extra.md"
  ],
  "paths": [
    "src/shop/**",
    "features/cart.feature"
  ],
  "resumable": false,
  "state": "closed",
  "title": "Show the cart total",
  "type": "Feature",
  "version": "0.1.0"
}
```

| Field | Type | Written by | Meaning |
|---|---|---|---|
| `id` | string | `gdd new-change` | The change id: a lowercase letter or digit, then up to 40 lowercase letters, digits or hyphens. Its branch is `change/<id>`. |
| `title` | string | `gdd new-change` | What the change is about. |
| `type` | string | `gdd new-change` | `Feature`, `Hotfix`, `Refactor`, `Migration`, `Spike` or `No-code`. |
| `paths` | array of strings | `gdd new-change` | The paths it plans to touch; patterns allowed. |
| `resumable` | boolean | `gdd new-change` | `--resumable`: a condition of the Fast Lane. |
| `min_color` | string or null | `gdd new-change` | `--min-color`: raises the color, never lowers it. |
| `created` | string | `gdd new-change` | When it was created, UTC. |
| `state` | string | gdd | See [States](#states). |
| `gates` | object | `gdd gate`, the recipe | One entry per gate decided on the change: see below. |
| `inbox` | array | `gdd note`, `gdd park`, the recipe | The notes of the Decision Inbox: see below. |
| `variants` | array | `gdd new-change`, the recipe | Only a spike with variants: see below. |
| `decision` | object | `gdd prepare` | What prepare decided: see below. |
| `build_start` | string | `gdd prepare` | The commit where the build started: Path Guard and the scope check judge from it. |
| `features` | array of strings | `gdd gate G1` | The scenario files sealed at G1, hidden examples included. |
| `out_of_scope` | array of strings | the gates in build mode | Paths outside the plan the scope check accepted. |
| `reviews` | array | the recipe | Every Reviewer call: see below. |
| `spent_usd` | number | the recipe | What the change's agent calls cost, as the agents reported it. |
| `spent_tokens` | object | the recipe | Their tokens: `in`, `cache`, `out`. |
| `changelog` | object | `gdd changelog-line` | `{"section", "line", "breaking"}`, or `{"none": true}` to leave the change out. Without it, the title goes under the section of the type (Feature: Added, Hotfix: Fixed, Migration: Changed); other types have no line. |
| `commit` | string | `gdd merge` | The merge commit. |
| `version` | string | `gdd release promote` | The version the change was released as, without its `v`. |

Records written by an older tool may have other fields; gdd ignores them.

### gates

| Key | Present on |
|---|---|
| `plan` | a red change, once the human approved its plan |
| `G1` | a change with new or changed scenarios, once the human decided |
| `G2` | when recorded with `gdd gate G2` |
| `G3` | once the machines opened it (`automatic chain and Reviewer`) |
| `G4` | once the human answered, or the machines approved the promotion |

Each entry has `decision` (`ok` or `ko`), `by` (`human` or `machines`), `ts` and `note`. `G1`
also has `hashes`: each sealed scenario file and its SHA-256.

### inbox

Each note has `kind`, `text` and `ts`, and `resolved` (when it was resolved) once a relaunch
resolved it. `kind` is `needs` (a decision for the human), `decided` (taken without them),
`parked` (why it stopped) or `polish` (the human's notes at G4).

### decision

The classification of the change when it was prepared, and the model of each role.

| Key | Meaning |
|---|---|
| `change` | The id. |
| `color`, `lane` | `green`, `yellow` or `red`; `fast` or `full`. |
| `reasons` | One line per path: its color and why. |
| `g1`, `features` | Whether it needs G1, and its scenario files. |
| `tests_first`, `mutation`, `reviewer`, `review`, `g4`, `human_plan` | What its rule gives it (see [Gates](gates.md#gates-by-color)). |
| `type` | Its kind of work. |
| `builder`, `builder_test`, `reviewer_model`, `orchestrator` | The role tables of `gdd/models.toml` it got. |
| `builder_session` | `separate` or `shared`. |
| `retries` | Rounds of build mode against G2. |
| `resume_green` | A relaunch whose code already passes G2: no Builder. |

### reviews

One entry per Reviewer call, in order, relaunches included.

| Key | Meaning |
|---|---|
| `output` | Where its answer is kept, from the change's folder: `outputs/reviewer-<n>.md`. |
| `verdict` | `OK` or `KO` once the call ended; `null` when the call stopped the recipe. |
| `changed` | The paths whose hash changed since the review before; `null` for the first. |
| `hashes` | The hash of each path the change touched, right before the call. |

### variants

One entry per variant of a spike: `option` (the option it builds, or `null` for the same
brief), `branch` (`change/<id>-v<n>`), and once built `state` (`ready` or `parked`), `reason`,
`g2`, `adrs`, `decision` (the first line of its ADR's decision), `files`, `minutes`, `usd` and
`tokens`.

### States

| State | Meaning |
|---|---|
| `open` | Created, not prepared. |
| `building` | Prepared and in Build, a polish pass after G4 included. |
| `parked` | Stopped, with a message in its Decision Inbox. |
| `merged` | Merged into main. |
| `in-pre` | In pre, waiting for G4 or its promotion. |
| `promoting` | Its promotion is running. |
| `in-production` | Promoted, being observed. |
| `closed` | Observed: done. |
| `rejected` | KO at G4. |
| `rolled-back` | Observation failed and production rolled back. |

## inbox.md

The Decision Inbox: one message per change that gathers what the agents decided and what waits
for you. gdd renders it again from the record and the gate reports each time they change.
`gdd inbox <id>` prints it.

```text
# Decision Inbox · cart-total · Show the cart total

Color yellow · lane full · state closed

## Needs your decision
- Should the total show taxes

## Decided without you
- Kept the total as an integer
- Paths outside the package, same or lower color, accepted by rule: docs/extra.md

## Parked
- Nothing.
```

`Needs your decision` is the only group that waits for you. The groups, in order, each only
when it has something (the three note groups always show): `How it works` (from the Reviewer's
OK), `Variants`, `Needs your decision`, `Decided without you`, `Parked`,
`Security exceptions this change adds`, `Known debt`, `Known debt paid`, `Polish`; then how
many earlier requests were resolved before the last launch.

## Gate reports

`g2.json` and `g3.json` hold the last run of each gate on the change.

```json
{
  "ok": true,
  "gate": "G2",
  "color": "yellow",
  "passed": [
    "lint",
    "unit",
    "path-guard",
    "scope"
  ],
  "skipped": [],
  "failures": [],
  "out_of_scope": [
    "docs/extra.md"
  ]
}
```

| Field | Meaning |
|---|---|
| `ok` | Nothing failed. |
| `gate`, `color` | `G2` or `G3`; the change's color. |
| `passed` | The checks that passed, in order: commands, `path-guard`, `scope`, `scenarios (<n>)`, `exceptions`. |
| `skipped` | The `mutation*` commands the change does not need. |
| `failures` | Each with `name` and `tail` (what to read); a command also has `ok`, `code` (124 after a timeout) and `duration_s`. |
| `out_of_scope` | In build mode, the paths outside the plan the scope check accepted. |
| `added_exceptions` | G3 only, when any: the security exceptions the change adds. |
| `known_debt` | When any: commands with known debt that failed, with `name`, `tail`, and `count` and `known` when counted. |
| `paid_debt` | When any: commands with known debt that passed. |
| `summaries` | When any: the last line of each `mutation*` command that passed. |

## log.jsonl

The log behind every report: one JSON object per line, appended under a lock, so the lines of
two runs never mix. gdd writes a line for every agent call, gate decision, gate command, Path
Guard run and machine step. `gdd session` adds the interactive Claude Code sessions. Reports
leave out the lines that are not gdd's (not JSON, or a cost that is not a number of zero or
more) and say how many.

```json
{"ts":"2026-10-30T13:11:46+00:00","change":"cart-total","phase":"2-build","gate":"G2","role":"machine","agent":"lint","model":"-","tokens_in":0,"tokens_cache":0,"tokens_out":0,"cost_usd":0.0,"duration_s":0.0,"result":"ok","retry":0,"note":""}
{"ts":"2026-10-30T13:11:46+00:00","change":"cart-total","phase":"2-build","gate":"G2","role":"machine","agent":"checks","model":"-","tokens_in":0,"tokens_cache":0,"tokens_out":0,"cost_usd":0.0,"duration_s":0.0,"result":"ok","retry":0,"note":""}
```

The first line is the `lint` command of G2; the second is the gate as a whole (`checks`).

| Field | Type | Meaning |
|---|---|---|
| `ts` | string | UTC to the second: `2026-10-30T13:11:46+00:00`. |
| `change` | string | The change id; `-` for the project (G0, the red zones). |
| `phase` | string | `0-setup`, `1-discover`, `2-build`, `3-integrate`, `4-release`, `5-learn`, or `-`. |
| `gate` | string | `G0`, `zones`, `plan`, `G1` to `G4`, or `-`. |
| `role` | string | `orchestrator`, `builder-test`, `builder-build`, `reviewer`, `machine`, `human`, or who recorded a gate (`--by`, such as `machines`). |
| `agent` | string | The agent of a call (`claude`, `codex`, `opencode`, `claude-interactive`); for machine steps what ran: a command name, `checks` (the gate), `path-guard`, `selftest`, `verdict`, `merge`, a release step; `-` otherwise. |
| `model` | string | The model of an agent call; `-` otherwise. |
| `tokens_in` | integer | Input tokens not read from the cache. |
| `tokens_cache` | integer | Input tokens read from the cache. |
| `tokens_out` | integer | Output tokens. |
| `cost_usd` | number | Cost of the call: as the agent reported it, or estimated from `[prices]`. |
| `duration_s` | number | Seconds, to a tenth. |
| `result` | string | `ok` or `ko`; for an agent call `ok`, `error-<code>` or `timeout`; for a release step `ok`, `ko` or `simulated`; for a step, what happened: `change-created`, `prepared`, `parked`, `changelog-line`, `released`, `polish`. |
| `retry` | integer | For an agent call, the earlier calls of its role on the change; 0 otherwise. |
| `note` | string | What to read: the failed checks of a gate, the note of a decision, the command an agent ran (`/gdd-build`), the version released. |

## The merge commit

`.gdd/` never leaves your machine, so `gdd merge` puts what matters of the record into git:
it merges a change into the main branch with a commit whose message says what the change was
and who approved it. The merge commit of the same change, trimmed:

```text
gdd(cart-total): G3 Merge Gate · Show the cart total

Color: yellow · lane full · type Feature
Approvals: G1 ok (human, 2026-10-30, 1 scenario file(s) sealed)
Decided without the human:
  - Kept the total as an integer
...
Record: .gdd/changes/cart-total/ (local)

Changelog: Added · The cart shows its total
Breaking: no
```

| Line | When |
|---|---|
| `gdd(<id>): G3 Merge Gate · <title>` | Always. |
| `Color: <color> · lane <lane> · type <type>` | Always. |
| `Approvals: ...` | The red zones (red changes), G1 and the plan when the record has them; else `Approvals: none needed`. |
| `Reviewer: KO -> OK`, then its findings | When the change was reviewed. |
| `Decided without the human:` | The last eight `decided` notes. |
| `Tokens: in <n> · cache <n> · out <n> · <usd> USD (estimate) · <n> agent call(s)` | When the log has calls for the change. |
| `Known debt paid: ...`, `Known debt lowered: ...` | When the change paid or lowered known debt. |
| `Record: .gdd/changes/<id>/ (local)` | Always. |
| `Changelog: <section> · <line>` and `Breaking: yes` or `Breaking: no` | Git trailers, in a paragraph of their own, when the change has a changelog line. |

`gdd changelog` and `gdd release` read the `Changelog:` and `Breaking:` trailers of the merge
commits on main to write `CHANGELOG.md` and to pick the next version.

## See also

- [Configuration](configuration.md): the files in `gdd/`.
- [Environment variables](environment.md): what the commands and agents that write these files
  receive.
- [Get a parked change going again](../how-to/parked-change.md): reading the Decision Inbox.
- [Measure the work](../how-to/measure.md): reports built from `log.jsonl`.
