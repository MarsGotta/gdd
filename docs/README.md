# gdd documentation

gdd runs GDD, Gate-Driven Development, a way to build software with AI coding agents. Every
change moves forward by passing *gates*, checks it must pass before its next step. Risk decides
how many gates a change meets and how strict they are. Machines check what can be checked, and
you decide only what needs a person: the scenarios before the build and the result at the end.

*Esta documentación también está [en español](es/README.md).*

## Where do you start?

- **New project:** the [Quickstart](getting-started/quickstart.md) goes from installing gdd to a
  first released change, through your agent.
- **Existing code:** [Bring gdd to an existing project](how-to/adopt-existing-project.md) turns
  what fails today into known debt and gives your folders their risk colors.
- **One whole change, step by step:** the tutorial
  [Your first change](getting-started/tutorial.md) shows one real change and every screen on
  the way.
- **The method first:** [GDD in one page](explanation/method.md) explains the phases, the gates
  and the traffic light.
- **A command:** the [command-line (CLI) reference](reference/cli/index.md) lists every `gdd`
  command, its options and what it prints.
- **A skill:** the [skills reference](reference/skills/index.md) says what `/gdd-setup`,
  `/gdd-change` and the other skills do, and when to run each.

## How you use gdd

- **Terminal,** once per project: you install gdd and run `gdd init`. Its form sets up the
  project, installs its stacks and then tells you to start your agent.
- **Agent chat,** for everything else: `/gdd-setup` once, then `/gdd-change` for each change.
  In Codex the skills start with `$`: `$gdd-setup`, `$gdd-change`.
- **Approvals,** in the chat or on the sitting screen (`gdd sitting CHANGE`): the scenarios
  before the build, and OK, KO or polish on the result at the Release Gate (G4).
- **Board,** to watch if you want: `gdd` alone in a terminal, or `gdd board`. It does not
  refresh by itself; to see what moved, open it again.

## Find your way around

- **Get started:** [Quickstart](getting-started/quickstart.md),
  [Your first change](getting-started/tutorial.md) and [GDD in one page](explanation/method.md).
- **Guides,** named by what you want to do:
  [Describe a change to your agent](how-to/run-a-change.md),
  [Approve the scenarios](how-to/write-scenarios.md),
  [Review a change at G4](how-to/release-gate.md),
  [Get a parked change going again](how-to/parked-change.md),
  [Install gdd](how-to/install.md),
  [Bring gdd to an existing project](how-to/adopt-existing-project.md),
  [Map and approve red zones](how-to/red-zones.md),
  [Explore with a spike](how-to/spikes.md),
  [Run changes in parallel](how-to/parallel-changes.md),
  [Choose agents and models](how-to/agents-and-models.md),
  [Tune the gates](how-to/gates-and-commands.md),
  [Run the gates in CI](how-to/ci.md),
  [Upgrade gdd in a project](how-to/upgrade.md),
  [Release versions and keep a changelog](how-to/releases-and-changelog.md),
  [Measure the work](how-to/measure.md) and
  [Troubleshooting](how-to/troubleshooting.md).
- **Reference,** exact and complete:
  - [Skills](reference/skills/index.md): [`gdd-setup`](reference/skills/setup.md),
    [`gdd-discover`](reference/skills/discover.md), [`gdd-change`](reference/skills/change.md),
    [`gdd-status`](reference/skills/status.md), [`gdd-report`](reference/skills/report.md),
    [`gdd-learn`](reference/skills/learn.md), [`gdd-hotfix`](reference/skills/hotfix.md) and
    [Skills of gdd's agents](reference/skills/agent-skills.md).
  - [CLI reference](reference/cli/index.md), one page per command:
    [`init`](reference/cli/init.md), [`bootstrap`](reference/cli/bootstrap.md),
    [`adopt`](reference/cli/adopt.md), [`agents`](reference/cli/agents.md),
    [`agents install`](reference/cli/agents-install.md),
    [`agents check`](reference/cli/agents-check.md), [`stacks`](reference/cli/stacks.md),
    [`stacks show`](reference/cli/stacks-show.md),
    [`stacks compose`](reference/cli/stacks-compose.md), [`upgrade`](reference/cli/upgrade.md),
    [`new-change`](reference/cli/new-change.md), [`classify`](reference/cli/classify.md),
    [`sitting`](reference/cli/sitting.md), [`gate`](reference/cli/gate.md),
    [`note`](reference/cli/note.md), [`changelog-line`](reference/cli/changelog-line.md),
    [`launch`](reference/cli/launch.md), [`resume`](reference/cli/resume.md),
    [`park`](reference/cli/park.md), [`prepare`](reference/cli/prepare.md),
    [`merge`](reference/cli/merge.md), [`release`](reference/cli/release.md),
    [`changelog`](reference/cli/changelog.md), [`g2`](reference/cli/g2.md),
    [`g3`](reference/cli/g3.md), [`guard`](reference/cli/guard.md),
    [`snapshot`](reference/cli/snapshot.md), [`scope`](reference/cli/scope.md),
    [`selftest`](reference/cli/selftest.md), [`ci`](reference/cli/ci.md),
    [`status`](reference/cli/status.md), [`board`](reference/cli/board.md),
    [`inbox`](reference/cli/inbox.md), [`report`](reference/cli/report.md),
    [`learn`](reference/cli/learn.md) and [`session`](reference/cli/session.md).
  - [Screens](reference/screens.md), [Configuration](reference/configuration.md),
    [Gates](reference/gates.md), [Roles](reference/roles.md), [Stacks](reference/stacks.md),
    [Files gdd writes](reference/files.md),
    [Environment variables](reference/environment.md) and the [Glossary](glossary.md).
- **Concepts,** why GDD works the way it does:
  [The life of a change](explanation/life-of-a-change.md),
  [How risk picks the gates](explanation/risk.md),
  [Keeping agents honest](explanation/safety.md),
  [Few stops for the human](explanation/human-stops.md),
  [Measuring what matters](explanation/measuring.md) and
  [Design decisions](explanation/design-decisions.md).

## This project, built with GDD

gdd is built with GDD, so its own GDD files are real examples of every artifact:

- [The method's full rules](../GDD.md) and the [roadmap](../ROADMAP.md).
- [Project Brief](brief.md), [standards](standards.md), [decisions](adr/) and
  [Discover notes](discover/).
- [Scenarios](../features/): the executable specification of gdd.

## Contribute

- [Contributing](../CONTRIBUTING.md): set up, test and send a change.
- [Writing gdd's documentation](STYLE.md): how these pages are organized and written.
- [Changelog](../CHANGELOG.md).
