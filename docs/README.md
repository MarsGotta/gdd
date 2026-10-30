# gdd documentation

gdd runs GDD, Gate-Driven Development: a way of building software with AI coding agents in
which every change moves forward by passing gates. Risk decides how many gates a change meets
and how strict they are. Machines check what can be checked; you decide only what needs a
person, in as few sittings as possible.

New here? Start with the tutorial: in about half an hour you set up a small project, approve
its first scenarios and watch the agents build, review and release a change.

**[Your first change →](getting-started/tutorial.md)**

*Esta documentación también está [en español](es/README.md).*

## Learn

Step by step, from nothing to a released change.

- [Your first change](getting-started/tutorial.md): set up a project, write and approve a
  scenario, launch the change and answer its Release Gate.
- [GDD in one page](explanation/method.md): the phases, the gates and the traffic light, with
  a diagram of each.

## Do

Guides for a goal you already have.

**Day to day**

- [Install gdd](how-to/install.md): the binary, the agents and the tools the gates use.
- [Run a change](how-to/run-a-change.md): create it, launch it and follow it on the Board.
- [Write and approve scenarios](how-to/write-scenarios.md): Gherkin, hidden examples and the
  Scenario Gate (G1).
- [Answer the Release Gate](how-to/release-gate.md): OK, KO or polish at G4.
- [Get a parked change going again](how-to/parked-change.md): the Decision Inbox, relaunches
  and merge conflicts.

**Shaping the work**

- [Map and approve red zones](how-to/red-zones.md): tell gdd which paths are risky.
- [Explore with a spike](how-to/spikes.md): a fixed budget, variants side by side, an ADR.
- [Run changes in parallel](how-to/parallel-changes.md): the cap, overlaps and the merge queue.

**Setting up and tuning**

- [Bring gdd to an existing project](how-to/adopt-existing-project.md): init, bootstrap and adopt.
- [Choose agents and models](how-to/agents-and-models.md): who builds, who reviews, per color.
- [Tune the gates](how-to/gates-and-commands.md): the commands each gate runs, limits and mutation testing.
- [Release versions and keep a changelog](how-to/releases-and-changelog.md): tags, `gdd release`, CHANGELOG.md.
- [Run the gates in CI](how-to/ci.md): what the workflow checks on every pull request.
- [Measure the work](how-to/measure.md): tokens, flow time, DORA and Learn.
- [Upgrade gdd in a project](how-to/upgrade.md): new versions of the files gdd wrote.
- [Troubleshooting](how-to/troubleshooting.md): symptoms, causes and fixes.

## Look up

Exact and complete. The command reference is generated from the code.

- [Command reference](reference/cli.md): every command of `gdd` and its options.
- [Configuration](reference/configuration.md): every file in `gdd/` and every key.
- [Gates](reference/gates.md): what each gate checks, who decides and its limits, by color.
- [Roles](reference/roles.md): the human, the Orchestrator, the Builder and the Reviewer.
- [Stacks](reference/stacks.md): the Python, Rust and TypeScript sheets.
- [Files gdd writes](reference/files.md): records, the log, the Decision Inbox and outputs.
- [Environment variables](reference/environment.md): what gdd reads and what it gives agents.
- [Glossary](glossary.md): the words these pages use.

## Understand

Why GDD works the way it does.

- [GDD in one page](explanation/method.md): phases, gates, colors and roles.
- [The life of a change](explanation/life-of-a-change.md): every step of a launch, in order.
- [How risk picks the gates](explanation/risk.md): the traffic light and why color wins.
- [Keeping agents honest](explanation/safety.md): Path Guard, hidden examples and what the agents cannot touch.
- [Few stops for the human](explanation/human-stops.md): sittings, polish and why done is not good.
- [Measuring what matters](explanation/measuring.md): tokens, DORA and the kill rule.
- [Design decisions](explanation/design-decisions.md): what was chosen, and what was not.

## This project, built with GDD

gdd is built with GDD, so its own GDD files are real examples of every artifact:

- [The method's full rules](../GDD.md) and the [roadmap](../ROADMAP.md).
- [Project Brief](brief.md), [standards](standards.md), [decisions](adr/) and
  [Discover notes](discover/).
- [Scenarios](../features/): the executable specification of gdd.

## Contribute

- [Contributing](../CONTRIBUTING.md): set up, test and send a change.
- [Writing the docs](STYLE.md): how these pages are organised and written.
- [Changelog](../CHANGELOG.md).
