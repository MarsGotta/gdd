# Design decisions

The choices that shape GDD and gdd, and the alternatives that were turned down, with the reason
for each. Read it when something in GDD looks stricter or simpler than you expected and you want
to know whether that was on purpose.

## Before you start

[GDD in one page](method.md) gives the method. The project's own records hold the full
reasoning: the technology decision in [ADR 0001](../adr/0001-technology.md), the
[Discover notes](../discover/) and the [roadmap](../../ROADMAP.md). This page gathers the
decisions a newcomer asks about most.

## One Rust binary

**Chosen.** gdd is a single Rust binary, built as a Cargo workspace: a library with the method
(`gdd-core`, with no terminal or command-line dependencies), the binary with the CLI and the
terminal UI, and the acceptance scenarios, which talk to the library, not to the screen.

**Why.** gdd launches agents, runs gate commands and keeps state on disk, and it is published as
open source, so installing it must take one step. One static binary installs anywhere without a
Python or Node runtime.

**Rejected.** Python with Textual was fastest to write, since the first version of the logic
already existed in Python, but it needs a Python runtime to install. Go with Bubble Tea also
gives one binary, but the author's other projects already used Rust and none used Go. The cost of
Rust was accepted knowingly: compile times make G2 slower, so the acceptance scenarios stay at
the library level to keep it fast.

## Gherkin is required; the runner is yours

**Chosen.** GDD imposes the method, not the tool. Every project writes its scenarios in
[Gherkin](https://cucumber.io/docs/gherkin/reference/), the language of `Given`, `When` and
`Then`, in `.feature` files under `features/`. Each project picks the runner that fits its
technology, among runners with broad community support: the official Cucumber implementation or
the de facto standard of its ecosystem. Whatever the runner, it must:

- **fail on undefined or pending steps** (strict mode), so a sentence nobody bound to code can
  never pass;
- **run only some `.feature` files**, so G2 runs the change's scenarios and G3 runs all of them;
- **run the hidden examples** in `features/hidden/`.

**Why Gherkin.** Scenarios are the contract you approve without reading code, so they must be
sentences a person can judge, and also the test that runs. That is the promise of behaviour-driven
development, in Cucumber's words: "system documentation that is automatically checked against
the system's behaviour" ([Cucumber, BDD](https://cucumber.io/docs/bdd/)).

**Why any runner.** A method that required one runner would exclude every stack that runner fits
badly. The three requirements are what GDD's gates need; the rest is the project's business.

**Rejected.** An early version of the method required Cucumber itself, which tied it to one family
of tools. A "Gherkin catalog" was also turned down: plain tests named after each scenario's id,
with a script that checks every id has one. It proves a test *exists*, not that it checks what
the approved sentence says, and an agent could ship a weaker test under the right name. Running
the approved Gherkin is part of the core.

**The usual critique, answered.** The step layer of BDD tends to grow and rot. GDD fights that
with instructions, not tools: test mode reuses a step that already says the same and adds one
only when none fits, and drives each step at the lowest layer that shows the behavior (a domain
function, then the command line, then the screen). The Reviewer names a duplicated step as a
finding. Only a few scenarios, tagged `@ui`, go through the screen; the demo in pre checks the
real interface.

## Minimal by default, incremental by choice

**Chosen.** Three levels, and only the first is fixed:

1. **The core** that guards against the agents is always on, and no Builder can turn it off: the
   gates G1 to G4, the traffic light, Path Guard, hidden examples, a Reviewer of another model
   family, and mutation testing on red changes. A rules file that switches the core off for red
   changes is refused; a key it leaves out counts as on.
2. **The stack's checks** are on by default and can be switched off: the linter, the types, the
   unit tests, the scanners of the project's stack sheet.
3. **Extras** are opt-in, each by adding its section to a `gdd/*.toml`: mutation testing on yellow
   changes, a deep review for yellow, the Learn tools in `[learn]`, one shared Builder session.
   None of them adds a question to `gdd init`.

**Why.** A method that asks a new user to decide twenty things before the first change is a method
nobody starts. A method whose safety can be configured away is no safety. The levels separate the
two: what protects you cannot be removed, and what costs time or tokens is yours to add when it
pays.

**Rejected.** Mutation testing on every yellow change by default. It is still the rule in
[`GDD.md`](../../GDD.md#gate-level-traffic-light) and in gdd's own repository, but it is a slow
gate (gdd needed a change of its own to speed it up for Rust), and minimal by default moved it:
to the core for red changes, and to one line (`mutation = true`) for yellow.

## Roles with minimum context

**Chosen.** Each agent gets only what its task needs. The Orchestrator holds the whole map and
writes, for each change, a small **package**: the goal in three lines, the color, the scenarios,
the paths it may touch, the contracts and functions to reuse, the rules of its zone and the G2
command. The Builder reads the package, not the map.

**Why.** A smaller context is cheaper and more focused, and it is also a safety measure: the
Builder's package leaves out the hidden examples.

Splitting the roles also puts a second model between the work and main. The Builder and the
Reviewer form what Anthropic calls an *evaluator-optimizer* workflow, "one LLM call generates a
response while another provides evaluation and feedback in a loop", which it recommends "when we
have clear evaluation criteria" ([Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)).
In GDD the criteria are the scenarios, the package and `docs/standards.md`, and the loop has a
limit: one fix.

**The alternative**, one agent that plans, builds and reviews, is simpler, but it grades its own
work, and its context grows with every change.

## Deterministic tools before agents

**Chosen.** Machines decide what can be checked, and those machines are deterministic tools
first: the linter, the type checker, the tests, the scenarios, mutation testing, the security and
license scanners, secrets detection. An agent judges only what no tool can, and a person only
what no agent should.

**Why.** A tool gives the same answer every time and says why; an agent may not. Anthropic's
guidance on agents makes the same point from the other side: it is "crucial for the agents to gain
'ground truth' from the environment at each step (such as tool call results or code execution)"
([Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)).
The gates are that ground truth. Learn pushes the same way: every repeated error becomes a rule,
a lint, a hook or a test, so the next time a tool catches it.

**Rejected.** An agent as the security reviewer of every change. Today only the scanners look at
security on their own; a separate security review by an agent, for red security zones, is planned,
and it will add to the scanners, not replace them.

## Separate test and build modes

**Chosen.** The Builder works in two modes, in separate sessions by default. Test mode writes the
step definitions for the approved scenarios, with real assertions, and must see them fail because
the behavior is missing. Build mode writes the code until G2 is green, and may not touch the steps.

**Why.** If the agent that writes the code also writes the tests that judge it, it can make both
agree in the wrong way. Splitting the modes, and letting Path Guard check each one against its own
snapshot, means the tests exist before the code and cannot be bent to it afterwards.

**The opt-in shared session.** Two sessions lose context at the handoff. An experiment on gdd's own
changes measured it: one shared session spent about 21% fewer new tokens and about 7% less time,
with quality within the noise. That was not enough to change the default, so it became a choice:
`[session] builder = "shared"` in `gdd/models.toml`, available for Claude Code. Path Guard still
proves the steps are not edited after test mode. [Measuring what matters](measuring.md#an-example-one-builder-session-or-two)
has the numbers.

**Rejected.** One mode that writes the tests and the code together, in one go and with no check
between them.

## Spikes whose code never merges

**Chosen.** A **spike** answers one question so that you can decide. It is built fast, without
tests first, mutation testing or the Reviewer, within a fixed budget (`[spike] max_usd`, 5 USD by
default). Its gate is Path Guard and an ADR with a `## Decision` section. OK at G4 brings only the
ADR to main; the code stays on its branch. A spike may build two to four variants, one per option,
for you to compare on one page.

**Why.** Exploration code is written to learn, not to last. If it could merge, it would, and the
project would keep code nobody tested for real. Keeping only the decision makes the spike cheap to
start and safe to throw away.

**Rejected.** Merging a spike's code as it is; gdd's own [standards](../standards.md) say it
plainly: "A spike's code does not enter main as it is." What the spike learned goes into a new
change, with its own scenarios and gates.

## Ideas from Spec Kit, no dependency on it

**Chosen.** gdd does not depend on [GitHub Spec Kit](https://github.com/github/spec-kit), the
toolkit for spec-driven development ("define what to build and why, then turn the specification
into a plan, tasks, and working code"). It borrows three of its ideas: one data sheet per agent,
commands written once as templates and rendered for each agent, and resumable runs with their
state on disk.

**Why.** GDD's first harness, a Python bootstrap kit, sat on top of Spec Kit, and the integration
brought three problems: commands registered only for the active agent, broken development-mode
links, and text-only capture that hid token usage. Owning the agent sheets and the recipe removed
all three, and let gdd start each agent itself, with the permissions of its role and without
secrets.

**Rejected.** Keeping Spec Kit underneath. The method's core, the gates and Path Guard, would
have depended on a layer gdd does not control.

## Colors, not work types

**Chosen.** Risk decides the gates. A change's color comes from the paths it touches, and **color
beats work type and lane**: a Hotfix in a red zone is red.

**Why.** The type says why you change something; the color says what breaks if the change is
wrong. Only the second should decide how careful to be. [How risk picks the gates](risk.md) has the
full argument.

**Rejected.** Lighter gates for Hotfixes or for small changes. Urgency is handled by rolling back
first, not by checking less.

## An English tool, agents in your language

**Chosen.** The tool, its files, its commands and its messages are in English. The agents talk to
you in the language set in `gdd/project.toml` (`language`): their questions, notes, summaries and
Decision Inbox entries.

**Why.** gdd is open source, and English is the shared language of its code, configuration and
issues. The person approving scenarios and answering G4, though, should do it in the language they
think in. Splitting the two keeps the tool consistent and the decisions comfortable.

**Rejected.** A translated tool. Every message would exist in many versions, and the scenarios
that check them would multiply.

## Smaller decisions

- **Overlapping changes warn instead of refusing.** At first, a launch whose planned paths
  overlapped a change in flight was refused. In gdd's own history every change touched the same
  steps file, so no two could ever run together. Now overlaps warn and git settles them at the
  merge, as a merge queue does; a conflict parks the change with the way out.
- **Worktrees in a sibling folder.** Each change builds in `<repo>.worktrees/<id>`, next to the
  repository, not inside it, where test runners and linters would find the copies. It is one line
  of configuration to change.
- **One model family, accepted in the open.** GDD wants the Reviewer from another family. With only
  one installed, `gdd init` refuses unless you accept it, records the date, and every G4 and merge
  commit says so. Requiring two families would leave out anyone with one subscription.
- **The changelog is the project's.** gdd proposes one user-facing line per change, carried in the
  merge commit, and adds to `CHANGELOG.md` only what it lacks, in
  [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format; it never rewrites the file.
  Versions follow [Semantic Versioning](https://semver.org/), tagged only after a promotion
  succeeds.
- **The record travels in git, the working files stay local.** The merge commit carries what
  happened; `.gdd/` holds the drafts, outputs and logs, and is not committed.
- **These docs follow [Diátaxis](https://diataxis.fr/)**: tutorials, how-to guides, reference and
  explanation, each page doing one job. This page is explanation.

## See also

- [ADR 0001 · Technology](../adr/0001-technology.md): the Rust decision in full.
- [Keeping agents honest](safety.md): the core these decisions protect.
- [How risk picks the gates](risk.md): why color decides.
- [Measuring what matters](measuring.md): how decisions are checked with numbers.
