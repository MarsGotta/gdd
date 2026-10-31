# Skills of gdd's agents

When gdd runs a change, it launches agents itself, headless (with no conversation and nobody
to answer), and hands each one a skill: the Builder in test mode, the Builder in build mode, the
Reviewer and, when the build is stuck, the Orchestrator for a replan. You never run these
skills. This page lists what each one is told to do; consult it when you read their work in
the Decision Inbox and the gate reports.

## How gdd hands them out

[`gdd launch`](../cli/launch.md) and [`gdd resume`](../cli/resume.md) start each agent with
its skill and a short input that gdd writes, such as `change=export-csv mode=build`. The agent
runs in the change's worktree, on its branch, with nobody to answer its questions. GDD state
is not in that worktree: it lives in `.gdd/` of the main checkout, whose path gdd passes in the
variable `GDD_STATE`. The agents read the state there and change it only through gdd commands.

Which agent and model plays each role comes from `gdd/models.toml`; the Reviewer is always of
another model family than the Builder. What each role may run is set per agent, in the agent
sheets built into gdd: see [Agent permissions](../roles.md#agent-permissions).

gdd also starts each agent with its role in the variable `GDD_ROLE`. An agent that gdd started
may only run `gdd g2`, `guard`, `note`, `status`, `classify`, `inbox`, `report` and `scope`;
gdd refuses every other command it tries. See
[What an agent may run](../roles.md#what-an-agent-may-run).

| Skill | Role | gdd launches it | Input after `change=<id>` |
|---|---|---|---|
| [`gdd-build-test`](#gdd-build-test) | Builder, test mode | first, when the change gets tests first; again after a replan that says `NEXT: test`, or after a Reviewer's KO with a finding on a file only test mode may write | `mode=test`, maybe `after=replan` or `fix=review` |
| [`gdd-build`](#gdd-build) | Builder, build mode | for each round against G2, and for each fix | `mode=build` or `mode=spike`, maybe `fix=g3`, `fix=review`, `fix=polish`, `after=replan`, `variant=<n>` |
| [`gdd-review`](#gdd-review) | Reviewer | after G3 passes | `round=1` or `round=2` |
| [`gdd-replan`](#gdd-replan) | Orchestrator, headless | when G2 is still red after the Builder's retries | `reason=g2` |

Each one talks in the language of `gdd/project.toml`. The Builders and the Reviewer first read
the stack instructions, `gdd-stack-<name>`, for every stack of the project.

## gdd-build-test

The Builder in test mode writes the steps of the scenarios and the tests, before any code.

- **Reads** the context package `.gdd/changes/<id>/package.md`, the `.feature` files it
  names, and the code under its paths, for reading only.
- **Writes** the step definitions in the words of the domain, with a real assertion in every
  `Then` step. It reuses a step that already says the same thing, and adds a new one only when
  none fits, with its values as parameters. Each step drives the behavior at the lowest layer
  that shows it: a domain function, then the command line, then the screen.
- **On a yellow or red change** that touches existing code, it first writes characterization
  tests that pin down what the code does today.
- **Checks** with [`gdd g2 --role builder-test`](../cli/g2.md): the new tests must fail
  because the behavior is missing, not because of a syntax or setup error.
- **After a replan** (`after=replan`), it fixes only what `replan.md` names, in the steps and
  tests.
- **After a Reviewer's KO** (`fix=review`), it fixes only the findings on the files test mode
  writes (the scenario steps and the tests written first), at every place of the same class.
  Build mode fixes the rest.
- **Never** changes a `.feature` file (you approved them at G1), writes production code,
  touches protected paths, or skips or silences tests.
- **Ends** with a three-line summary: which tests it wrote and why they fail now.

## gdd-build

The Builder in build mode writes the code until the Loop Gate (G2) passes.

- **Reads** the package, `docs/standards.md` when it exists, and, as the input says, the
  failures of the round before (`g2.json`), of G3 (`g3.json`, with `fix=g3`), the Reviewer's
  reasons for a KO (`review.md`, with `fix=review`), the replan (`replan.md`, with
  `after=replan`), or your polish notes from G4 in the inbox (`fix=polish`).
- **Writes** the domain code, its wiring into the application and the unit tests it needs. It
  reuses what the package says to reuse and adds no dependency: when one seems necessary, it
  leaves a note.
- **Checks** with [`gdd g2 --role builder-build`](../cli/g2.md) and fixes until G2 is green,
  or until it sees it cannot.
- **On a Reviewer's KO**, it fixes every place of each finding's class of bug, not only the
  line named, and lists the places it changed for each finding.
- **Never** edits acceptance tests or `.feature` files, touches protected paths, silences
  warnings or skips checks, or writes special cases for the scenario examples. A file outside
  the package's paths must stay at the change's color or lower and comes with a `decided`
  note; a path of a higher color fails the scope check of G2.
- **When something is ambiguous**, it picks the simplest reversible option and notes it with
  [`gdd note`](../cli/note.md): `decided`, or `needs` when you must decide.
- **Ends** with a three-line summary: what it changed and the state of G2.

With `mode=spike`, the change is a Spike: the Builder builds the smallest thing that answers
its question, measures it, and writes an ADR under `docs/adr/` with a `## Decision` section.
G2 is then Path Guard and that ADR, and the spike parks once its calls cost what
`[spike] max_usd` in `gdd/rules.toml` allows. With `variant=<n>`, it builds only its variant's
option. See [Explore with a spike](../../how-to/spikes.md).

## gdd-review

The Reviewer, a model of another family than the Builder, reads the whole change and answers
OK or KO. It only reads: if it changes a file, gdd parks the change.

- **Reads** the package, the record `change.json`, the change's `.feature` files, the whole
  diff against `main`, the G2 and G3 reports and `docs/standards.md`.
- **Checks** that every step does what its text says, with real assertions; that there are no
  special cases or shortcuts to make tests pass; that the change stays within its scope; on a
  red change, security (data, permissions, secrets, unchecked input); work that only looks
  done; and new steps that repeat an existing one.
- **Depth**: `review` in the change's classification says `quick` (the touched files, the
  package and the scenarios) or `deep` (also the code that calls what changed, the paths its
  data takes, security holes, data loss, concurrency, every red zone touched). Red changes
  always get a deep review.
- **Earlier reviews**: a later review says, for each earlier blocking finding, whether it is
  fixed, and looks for new findings only in the files that changed since. Round 2 checks the
  reasons of the round 1 KO and opens no new topic unless it blocks.
- **Answers** with findings as `F<n> · <class> · file:line · what fails · other places`, and
  non-blocking ones under «Improvements». On OK it adds a `## How it works` section, 3 to 6
  plain lines on where the change sits and how it fits: that is the *How it works* you read in
  the Decision Inbox and on the G4 sitting. The last line is `VERDICT: OK` or `VERDICT: KO`.

## gdd-replan

The Orchestrator, without a conversation, replans when the Builder used up its retries and G2
is still red.

- **Reads** the package, `g2.json` and the Builder's outputs in `.gdd/changes/<id>/outputs/`.
- **Decides the cause**: a task too big, a package with too little context, a wrong approach,
  or a harness problem.
- **Writes** `.gdd/changes/<id>/replan.md`: the cause in one sentence, the new approach in
  concrete steps, and any context the package lacked. The package itself stays as it is.
- **Ends** `replan.md` with `NEXT: test`, when the fault is in the steps or tests of test mode,
  or `NEXT: build`. gdd runs a test-mode round first on `NEXT: test`, then one more build
  round either way.
- **When only you can fix it**, a harness problem or a decision, it leaves a `needs` note
  instead of a fix. When G2 is still red after the last build round, the change parks, and the
  note is in its Decision Inbox.
- **Never** writes code, loosens gates, changes `gdd/` or touches approved tests.

## Stack instructions

[`gdd agents install`](../cli/agents-install.md) also writes one skill per stack in `stack` of `gdd/project.toml`, named
`gdd-stack-<name>`, such as `gdd-stack-python`: that stack's own instructions. The Builders and
the Reviewer read it before they work. [Stacks](../stacks.md) lists what each stack sheet
brings.

## See also

- [Skills](index.md): the skills you run yourself.
- [Roles](../roles.md): who builds, who judges, and what each may not do.
- [The life of a change](../../explanation/life-of-a-change.md): when each agent runs in the
  recipe.
- [Gate reports](../files.md#gate-reports): `g2.json`, `g3.json` and the other files the agents
  read.
