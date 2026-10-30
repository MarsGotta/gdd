# Configuration

Every file in `gdd/` and every key gdd reads from it: its type, the value gdd uses when the key
is missing, and what it does. The files are TOML.

## The files

`gdd init` writes the first five files and `gdd/generated.toml`; `gdd adopt` writes
`gdd/debt.toml`. All of them are committed with the project.

| File | Written by | Read by | Holds |
|---|---|---|---|
| [`gdd/project.toml`](#projecttoml) | `gdd init` | `gdd status`, the stack commands, the agents | The project basics |
| [`gdd/zones.toml`](#zonestoml) | `gdd init`, then the Orchestrator | `gdd classify`, the gates, Path Guard's scope check, the red zone approval | The zone map: which paths have which color |
| [`gdd/rules.toml`](#rulestoml) | `gdd init` | every command that classifies, gates, launches or merges | The rules table, limits, protected paths |
| [`gdd/models.toml`](#modelstoml) | `gdd init` | `gdd prepare`, `gdd launch`, `gdd resume`, `gdd status`, `gdd agents` | The agent and model of each role |
| [`gdd/commands.toml`](#commandstoml) | `gdd init` | the gates, `gdd release`, `gdd selftest`, `gdd learn`, `gdd adopt`, `gdd ci` | The gate, Learn and release commands |
| [`gdd/generated.toml`](#generatedtoml) | `gdd init`, `gdd upgrade` | `gdd upgrade` | What `gdd init` wrote once and what it was asked |
| [`gdd/debt.toml`](#debttoml) | `gdd adopt`, then each merge | the gates, `gdd ci`, `gdd status` | Known debt of an adopted project |

There is no `gdd/release.toml`: see [release.toml](#releasetoml).

A file gdd cannot parse stops the command with the file's name and the TOML error. Keys gdd
does not read are ignored, except where a section below says otherwise.

## Who may change them

Every path under `gdd/` is harness:

- `gdd init` puts `gdd/**` in `[protected] all` of `gdd/rules.toml`, so Path Guard fails any
  Builder or Reviewer that touches it, in G2 and in CI (`gdd ci` fails a change branch that
  changes a path of `all`).
- `gdd init` puts `gdd/**` in the red harness zone of `gdd/zones.toml`, so a change that plans
  it is red.

Only a person changes these files, on the main branch, after the human agrees: the Orchestrator
proposes, the human decides (the [red zones](gates.md#red-zones) and the plan of a red change
are approved at the sitting). A change that edits `gdd/zones.toml` again needs the red zones
approved again.

Path Guard judges a build with the `[protected]` lists of `gdd/rules.toml` as it is now and as
it was at the commit where the build started: deleting a protection, or the whole file, frees
no role from it.

## project.toml

The project basics. `gdd init` writes it from its flags. The file that marks the project root:
gdd looks for `gdd/project.toml` in the folder it runs in and the folders above it, and refuses
to run outside a project (`not a GDD project`).

Read by `gdd status` (`name`, `language`, `platform`), by every command that needs the stacks
(`stack`), and by the agents, whose commands tell them to read it (`language`,
`scenario_runner`).

```toml
name = "notes"
description = "A command-line notes tool"
language = "en"   # language the agents use with the human
platform = "cli"   # mobile | desktop | web | cli | other
stack = ["python"]   # the stack sheets (gdd stacks)
scenario_runner = "pytest-bdd"
```

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | `?` in `gdd status` | The project's name. |
| `description` | string | none | What the project is, in words. Written for the agents and the Brief; gdd does not read it. |
| `language` | string | `en` | The language the agents use with the human. `gdd init --language` sets it. |
| `platform` | string | `?` in `gdd status` | Where it runs. `gdd init` accepts `mobile`, `desktop`, `web`, `cli` or `other`. |
| `stack` | string or array of strings | none | The [stack sheets](stacks.md) of the project, each once. A missing or empty `stack` names none. Anything else (a number, a list with a number) is an error. |
| `scenario_runner` | string | none | The Gherkin runner. `gdd init` writes the runners of the stacks, joined with `, `. For the agents; gdd does not read it. |

## zones.toml

The zone map: which paths have which color. Read by `gdd classify`, `gdd new-change`, `gdd
prepare`, the gates (to classify the change and for the scope check), `gdd status` and `gdd
gate zones`. The Orchestrator maps the red zones at the end of Discover; the human approves
them (`gdd gate zones ok`). See [How risk picks the gates](../explanation/risk.md).

The color of a path is the highest color among the zones that match it; a path no zone matches
takes `default`. The color of a change is the highest color among its paths, raised to its
`--min-color` when that is higher.

Path patterns: `**` crosses folders (`docs/**` matches everything inside `docs`; `**/x` also
matches `x` at the root), `*` and `?` stay inside one folder. `*.md` matches `README.md` but
not `notes/readme.md`.

### Top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `default` | `"green"`, `"yellow"` or `"red"` | `"yellow"` | The color of a path no zone matches. `gdd init` writes `yellow`: code without a zone is code without known tests. |

### [[zone]]

One table per zone, in any number, in file order. When several zones of the same color match a
path, the first one gives its reason.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `color` | `"green"`, `"yellow"` or `"red"` | the `default` color | The color of the paths. Any other word is an error that names it. |
| `paths` | array of strings | `[]` | Path patterns, from the project root. |
| `reason` | string | `""` | Why these paths have this color. `gdd classify` and the record show it. |

`gdd init` writes three zones: `docs/**` and `*.md` green; the harness (`gdd/**`, `bin/**`,
`.github/**`, `.claude/**`, `.agents/**`, `.opencode/**`, `.gdd/bin/**`) red; the supply chain
of the stacks (for example `pyproject.toml`, `uv.lock`, `osv-scanner.toml`,
`osv-scanner-licenses.toml`) red.

The red zone approval keeps a hash of every `[[zone]]` whose color is `red`, with every key it
has. Any change to a red zone, an added key included, asks for a new approval.

## rules.toml

The rules table: which gates each color and lane asks for, where the scenarios live, the limits
of every loop and the paths each role must not touch. Read by `gdd classify`, `gdd new-change`,
`gdd prepare`, `gdd g2`, `gdd g3`, `gdd guard`, `gdd scope`, `gdd launch`, `gdd resume`,
`gdd merge`, `gdd release`, `gdd selftest` and `gdd ci`.

### Top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `main_branch` | string | `"main"` | The branch changes leave from and merge into. `gdd init` writes the branch checked out when it ran. |
| `push_main` | boolean | `false` | Push the main branch after a merge, and the tags after a release, when the repository has a remote. |

### [acceptance]

| Key | Type | Default | Meaning |
|---|---|---|---|
| `dir` | string | `"features"` | Folder of the scenario files (`.feature`). A `.feature` inside it that a change creates or changes needs the Scenario Gate (G1). Its `hidden/` subfolder holds the hidden examples. |
| `regression` | string | `"features/regression"` | Folder whose `.feature` files need no G1. |
| `required` | boolean | `true` | Zero scenarios in `dir` fail G3 even for a change without scenarios of its own. A change with scenario files of its own fails G3 with zero scenarios whatever this says. |

G3 counts the lines of the `.feature` files in `dir` and its subfolders that start with
`Scenario:`, `Scenario Outline:`, `Escenario:` or `Esquema del escenario:`.

### [limits]

| Key | Type | Default | Meaning |
|---|---|---|---|
| `builder_retries` | integer, 0 or more | `3` | Rounds of build mode against G2 before the replans. At least one round always runs. |
| `replans` | integer, 0 or more | `1` | Replans of a change still red in G2: each is one Orchestrator call, test mode again when the replan says `NEXT: test`, one more build round and G2. Then the change parks. |
| `agent_timeout_s` | integer, 0 or more | `3600` | Seconds an agent call may take, until its output closes. Then its process group is killed and the change parks. `gdd init` does not write it. |
| `command_seconds` | integer above 0 | `1800` | Seconds each gate command and each release command may take. A command that runs out of time is killed with everything it started and ends with exit code 124. Any other value is an error that names `limits.command_seconds`. |
| `reviewer_fixes` | integer | `1` in the file `gdd init` writes | Not read by gdd 0.1.0. The Builder gets one fix after a Reviewer KO and one after a red G3, always. |

### [spike]

| Key | Type | Default | Meaning |
|---|---|---|---|
| `max_usd` | number above 0 | `5` | What the agent calls of a spike may cost, in USD, before it parks. Each variant of a spike has the whole budget for its own calls. Checked after each call, from the cost the record keeps. Any other value is an error that names `spike.max_usd`. |

### [parallel]

Not written by `gdd init`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `max` | integer, 1 or more | `2` | How many changes may build at once. A change counts while its state is `building`. `gdd launch` refuses one more; a change in flight that plans an overlapping path only warns. |
| `folder` | string | `"../<project folder>.worktrees"` | The folder of the changes' worktrees, from the project root. Each change runs in `<folder>/<id>`. |

### [versions]

Not written by `gdd init`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `mode` | `"auto"` or `"manual"` | `"auto"` | `auto`: a promoted change with a changelog line gets the next version (`v<x.y.z>` tag) once its promotion passed. `manual`: promotions release nothing, and `gdd release` releases what is merged. Any other value is an error. |

### [color]

What each color asks for in each lane. A color has a table of its own, `[color.<color>]`, and
one per lane, `[color.<color>.fast]` and `[color.<color>.full]`. The rule of a color in a lane
is its lane table when it has one, else its color table. A lane table does not inherit from the
color table: a key it leaves out takes the default below. A color the file does not mention
gets the defaults.

When the file has no `[color]` section at all, gdd uses the table built into it instead:

| Color and lane | `tests_first` | `mutation` | `reviewer` | `g4_if_g1` |
|---|---|---|---|---|
| green fast | `false` | `false` | `false` | `false` |
| green full | `true` | `false` | `true` | `true` |
| yellow fast | `true` | `true` | `true` | `false` |
| yellow full | `true` | `true` | `true` | `true` |
| red full | `true` | `true` | `true` | `true` |

`gdd init` writes the same tables with `mutation = false` for yellow, and `review` for every
lane.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `tests_first` | boolean | unset: only when the change has new scenarios (G1) | The Builder in test mode writes steps and tests before build mode. A green change without G1 never gets test mode, whatever this says. |
| `mutation` | boolean | `false`; red: always `true` | The G2 and G3 commands whose name starts with `mutation` run. Otherwise they are listed as skipped. |
| `reviewer` | boolean | `true` | The Reviewer checks the change after G3. |
| `g4_if_g1` | boolean | `true` | A change with new scenarios (G1) waits for the human at the Release Gate (G4). A red change always does. |
| `review` | `"quick"` or `"deep"` | `"quick"`; red: always `"deep"` | How deep the Reviewer looks. Quick: the files the change touched, its package and its scenarios. Deep: that, plus the code around the change and what can go badly wrong there. |

The core of GDD cannot be switched off for red. A red table (with or without a lane) that says
`mutation = false`, `reviewer = false`, `g4_if_g1 = false` or `review = "quick"` refuses the
whole file:

```text
error: gdd/rules.toml: red changes always get mutation testing, the Reviewer and the human G4 (the core of GDD): remove color.red.full.mutation = false from gdd/rules.toml
```

A review depth other than `quick` or `deep`, and a color other than `green`, `yellow` or `red`,
are refused too. Red never travels the fast lane: `[color.red.fast]` is never used.

The lane of a change is `fast` when its color is not red, it has no new or changed scenarios
and it was created with `--resumable`; `full` otherwise. See [Lanes](gates.md#lanes).

### [protected]

The paths each role must not change, as patterns (see [zones.toml](#zonestoml)). Path Guard
checks them in G2 and G3, before and after the commands, after each test-mode round, and in
`gdd guard`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `all` | array of strings | `[]` | Paths no role may change: the harness. `gdd ci` fails a change branch that changes one. |
| `builder_build` | array of strings | `[]` | Paths the Builder in build mode must not change: the approved scenarios and the steps written in test mode. |
| `builder_test` | array of strings | `[]` | Paths the Builder in test mode must not change: the production code. |
| `reviewer` | array of strings | `[]` | Paths the Reviewer must not change. `gdd init` writes `["**"]`: everything. |
| any other key | array of strings | `[]` | The paths of the role with that name, `-` written as `_`. `builder` is the role `gdd g2`, `gdd g3` and `gdd guard` judge by default. |

A role must not touch the paths of `all` plus its own list. Independently of these lists, every
role fails Path Guard when a scenario file approved at G1 changed after the approval.

`gdd init` writes `all` as `gdd/**`, `bin/**`, `.github/**`, `.claude/**`, `.agents/**`,
`.opencode/**`, `.gdd/bin/**`, `AGENTS.md` and `CLAUDE.md`, and `builder_build` and
`builder_test` from the [stack sheets](stacks.md).

## models.toml

Who plays each role: an agent (integration) and its model. Read by `gdd new-change`,
`gdd classify` and `gdd prepare` (the model of each role goes into the decision of the change),
by `gdd launch` and `gdd resume` (which agent to start), by `gdd status` and by `gdd agents`.
`gdd init` writes it from `--agents` and the defaults of each agent; see
[Roles](roles.md#default-models).

```toml
[orchestrator]
integration = "claude"
model = "opus"

[builder.green]
integration = "claude"
model = "haiku"

[builder.yellow]
integration = "claude"
model = "sonnet"

[builder.red]
integration = "claude"
model = "opus"

[builder_test]
integration = "claude"
model = "sonnet"

[reviewer]
integration = "codex"
model = "gpt-6-astra"
effort = "high"

[families]
"claude" = "anthropic"
"codex" = "openai"
```

### [orchestrator], [builder.\<color\>], [builder_test], [reviewer]

The model of each role. `[builder.<color>]` is the Builder in build mode for changes of that
color (`green`, `yellow`, `red`); `[builder_test]` is the Builder in test mode for every color;
`[orchestrator]` replans a change stuck in G2; `[reviewer]` reviews every change that has a
Reviewer. A role without a table has no agent: its call fails.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `integration` | string | none | The agent: `claude`, `codex` or `opencode` (its agent sheet). |
| `model` | string | none | The model, as the agent names it. Passed to the agent's `--model` (`-m` in OpenCode). |
| `effort` | string | none | Reasoning effort, for agents whose sheet takes one: Codex gets `-c model_reasoning_effort=<effort>` (`low`, `medium`, `high`, `xhigh`). Claude Code and OpenCode ignore it. |
| `same_family_accepted` | string, `YYYY-MM-DD` | none | Only in `[reviewer]`: the day the human accepted a Reviewer of the Builder's model family. `gdd init --accept-same-family` writes it. Without it, `gdd prepare` refuses such a change. |

Other keys of a role table are kept, as they are, in the decision of the change.

### [families]

Which model family each model belongs to. The Reviewer must be of another family than the
Builder, so it does not share its blind spots.

| Key | Type | Default | Meaning |
|---|---|---|---|
| any key | string | none | `"<model name prefix or integration>" = "<family>"`. |

The family of a model is that of the first key, in file order, its model name starts with or
its integration equals; else the integration itself. `gdd init` writes one key per agent:
`claude` is `anthropic`, `codex` is `openai`, `opencode` is `minimax`.

### [session]

Not written by `gdd init`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `builder` | `"separate"` or `"shared"` | `"separate"` | `separate`: each Builder call starts a session of its own; build mode reads what test mode wrote. `shared`: each Builder call of a run continues the session of the Builder call before it, in either mode. Any other value refuses the file. |

A change gets a shared session only when `[builder_test]` and its `[builder.<color>]` use the
same integration and that agent's sheet says how to resume a session (Claude Code today); else
it runs separate and `gdd status` says why. The Reviewer and the Orchestrator never join it.
`gdd launch` and `gdd resume` read this table from the main checkout.

### [env]

Not written by `gdd init`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `pass` | array of strings | `[]` | Exact names of variables the agents get even though their name or their value says they hold a secret. See [Environment variables](environment.md#which-variables-hold-a-secret). |

### [prices.\<model\>]

Not written by `gdd init`. The price of a model, for agents that report no cost (Codex): one
table per model name, as `model` writes it.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `input` | number | `0` | USD per million input tokens not read from the cache. |
| `cache` | number | `0` | USD per million input tokens read from the cache. |
| `output` | number | `0` | USD per million output tokens. |

The estimate goes to the call's line in the log. The record of the change, and so the spike
budget, keeps only what the agent reported.

## commands.toml

The commands the machines run: the gate commands, the Learn tools, how to count known debt, the
self-test's failing test and the release steps. Every command is one line of shell, run with
`sh -c` from the project root, with nothing on its standard input; it passes with exit code 0.
`gdd init` writes this file from the [stack sheets](stacks.md) (what `gdd stacks compose`
prints) followed by the `[release]` section. Text an agent writes never reaches a shell: only
these commands do.

### Top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `install` | string | none | The install command of each stack, joined with ` && `. gdd does not run it: `gdd bootstrap` runs each sheet's own first install, and the CI workflow has its own install steps. |

### [g2]

The commands of the Loop Gate (G2), run on every Builder round and by `gdd g2`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| any name | string | none | A command. Commands run in the alphabetical order of their names. |

- A value that is blank, or not a string, is not configured: it never runs and is not listed.
- A gate without any configured command fails (`G2 not configured`).
- A command whose name starts with `mutation` runs only when the change needs mutation testing;
  otherwise it is listed under `skipped`.
- Every command gets `GDD_CHANGE`, `GDD_COLOR`, `GDD_DIFF_FILES` and `GDD_FEATURES`; see
  [Environment variables](environment.md#what-gate-commands-get).
- A failed command keeps, in the gate report, the first 40 lines of its output that report a
  failure (they start with `error` or hold `FAILED`, `Failed`, `failed`, `panicked`, `Error:`,
  `Traceback` or `✘`), then the last 2,500 characters.

The stack sheets name G2 commands `lint`, `types`, `unit`, `acceptance` and `architecture`.

### [g3]

The commands of the Merge Gate (G3), run by the recipe after G2 and by `gdd g3`. Same rules as
[`[g2]`](#g2). The stack sheets name them `acceptance_full`, `lockfile`, `security`,
`dependencies`, `secrets` and `mutation`. `gdd status` warns when no `[g3]` command starts
with `mutation`: red changes would go without mutation testing.

`gdd adopt` runs each G2 and G3 command once, except the ones whose name starts with
`acceptance`, `mutation` or `secrets`.

### [learn]

The tools `gdd learn` runs once on the project: warnings, never a gate.

| Key | Type | Default | Meaning |
|---|---|---|---|
| any name | string | none | A tool. Tools run in file order. Its name takes only letters, digits, `-` and `_`: what it says is kept in `.gdd/learn/<name>.txt`. Blank values and values that are not strings are left out. |

The stack sheets name them `duplication` and `unused`.

### [count]

How to count what a command with known debt reports, so its debt has a ceiling (a ratchet).

| Key | Type | Default | Meaning |
|---|---|---|---|
| a command name | array of strings | none | Texts to look for in the command's output. Each is not empty and has `{n}` once at most. A text with `{n}` adds up the number at each place it matches; a text without it counts the lines that contain it. Anything else is an error that names the command. |

A command with known debt that fails passes the gate while its count stays at or below the
count in [`gdd/debt.toml`](#debttoml). Reporting more, or a count gdd cannot read, fails the
gate.

### [selftest]

The deliberately failing test of the harness self-test (`gdd selftest`, check 1).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `file` | string | none | Where the failing test goes, from the project root. |
| `content` | string | none | The failing test. |

### [protected]

`gdd init` writes the `[protected]` table of the composed stack sheets here too. gdd does not
read it from this file: the lists Path Guard uses are in
[`gdd/rules.toml`](#protected).

### [release]

The release steps. Read by `gdd merge`, `gdd release` and `gdd selftest`. Each step runs its
command, never anything else, within `[limits] command_seconds`, with `GDD_CHANGE` and, once
the change has a version, `GDD_VERSION`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `simulated` | boolean | `false` | `true`: a step without a command passes and the log says `simulated`. `false`: a step without a command fails. `gdd init` writes `true`. |
| `build` | string | `""` | Builds the artifact once. Runs at the merge. |
| `deploy_pre` | string | `""` | Deploys the artifact to pre. Runs at the merge, after `build`. |
| `tests_pre` | string | `""` | Tests in pre. `gdd release pre`. |
| `demo` | string | `""` | The demo for G4: video, screenshots or the readable run of the scenarios, left in `.gdd/changes/$GDD_CHANGE/demo/`. `gdd release pre`, after `tests_pre`. |
| `promote` | string | `""` | Promotes the same artifact from pre to production. `gdd release promote`. |
| `observe` | string | `""` | Fails when errors rise in production. `gdd release observe`. |
| `rollback` | string | `""` | Goes back to the previous production version. Runs when `observe` fails. |
| `rollback_pre` | string | `""` | Rolls pre back. `gdd selftest` runs it for check 3; without it, check 3 passes only when `simulated = true`. |

A blank value counts as no command.

## release.toml

There is no `gdd/release.toml` in a project. The release steps live in the
[`[release]`](#release) table of `gdd/commands.toml`: `gdd init` appends them there from its
template.

## generated.toml

What `gdd init` wrote once and what it was asked, so `gdd upgrade` can render the same files
with a newer gdd and merge them with the project's edits. Written by `gdd init`; rewritten by
`gdd upgrade` on its `gdd-upgrade` branch. Do not edit it by hand.

### Top level

| Key | Type | Meaning |
|---|---|---|
| `version` | string | The gdd that wrote the files. |
| `files` | array of strings | The files `gdd init` writes once: `gdd/project.toml`, `gdd/commands.toml`, `gdd/rules.toml`, `gdd/zones.toml`, `gdd/models.toml`, `docs/brief.md`, `.github/workflows/gdd.yml`. |

### [answers]

| Key | Type | Meaning |
|---|---|---|
| `name` | string | `--name`. |
| `description` | string | `--description`; empty when not given. |
| `platform` | string | `--platform`. |
| `stacks` | array of strings | `--stack`, each once. |
| `agents` | array of strings | `--agents`, or the agents found on the `PATH`; the first builds. |
| `language` | string | `--language`. |
| `sensitive` | array of strings | `--sensitive`, one area each. |
| `accept_same_family` | boolean | `--accept-same-family`. |
| `day` | string | The day `gdd init` ran, `YYYY-MM-DD`. |
| `main` | string | The main branch it wrote into the files. |

`gdd upgrade` refuses a project without this file or without `[answers]`.

## debt.toml

The known debt of a project that already had code: the gate commands that failed when
`gdd adopt` ran. They warn instead of failing a gate until a change makes each pass. Written by
`gdd adopt`, which commits it alone and refuses to run once it exists; a merge removes the debt
its gates paid and lowers the counts its gates reported lower.

```toml
adopted = "2026-10-30"

[[debt]]
command = "lint"
gate = "g2"
since = "2026-10-30"
count = 3
```

### Top level

| Key | Type | Meaning |
|---|---|---|
| `adopted` | string | The day `gdd adopt` recorded it. |

### [[debt]]

| Key | Type | Meaning |
|---|---|---|
| `command` | string | The command's name in `gdd/commands.toml`. |
| `gate` | `"g2"` or `"g3"` | The section that has it. |
| `since` | string | The day it was recorded. |
| `count` | integer, optional | What the command reported, counted with [`[count]`](#count): its ceiling. Without it, the command only warns while it fails. |

A debt file gdd cannot read fails the gate check `debt`, and the first check of `gdd selftest`,
which holds G2 to the same debt.

## See also

- [Gates](gates.md): what each gate runs with these settings, by color.
- [Stacks](stacks.md): where the commands and protected paths of `gdd init` come from.
- [Tune the gates](../how-to/gates-and-commands.md): change commands and limits step by step.
- [Command reference](cli.md#gdd-init): the flags that fill these files.
