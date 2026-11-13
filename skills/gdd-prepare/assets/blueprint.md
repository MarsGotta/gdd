---
contract: gdd/1
guarantee: <rule, platform or engine>
levels:  # one block for each agents' environment the project is worked from
  <environment, by the name of its command>:
    launch: <how a role gets its model there, by its tool for launching agents or by a new session of its command line>
    light: <model for little reasoning, cheap and fast, by the name that environment launches it with>
    standard: <model for normal reasoning>
    deep: <model for the most reasoning>
    # reviewer:            # only for a reviewer on another family of models, once it was seen to work when preparing
    #   launch: <how, when it runs in another environment>
    #   light: <model>
    #   standard: <model>
    #   deep: <model>
limits:  # what holds for every color: the caps of one change are under each color
  lines_per_change: 400  # the builder's: git diff --shortstat <start> HEAD
  seconds_each_round: 120  # the checks that run each round, all together: one that takes longer runs before review
  seconds_before_review: 600  # the checks that run before the review, all together
  tokens_per_change: <n, where the environment reports them>
gates:  # the gates of the project, the same for every change, each one specified in its feature
  - id: <gate>
    spec: gdd/gates/<gate>.feature
    checks: [<check id>]
checks:
  - id: seal
    type: machine
    command: "git diff --exit-code <start> HEAD -- gdd features AGENTS.md CLAUDE.md .agents .claude <sealed paths> ':(exclude)gdd/log' ':(exclude)gdd/backlog.md'"
    runs: each round
  - id: tests
    type: machine
    command: <test command from the constitution>
    breaks: <the bad case to plant to see it fail>
    cases: each change  # a test for each scenario of the change's brief, seen failing and sealed before anyone builds
    runs: each round
    # passes: <what counts as passing, when exit 0 isn't all of it>
    # when: [':(glob)<pattern>']  # it applies only to a change that touches these paths
  - id: lint
    type: machine
    command: <lint and type check command from the constitution>
    runs: each round
  - id: secrets
    type: machine
    command: <secret scan command from the constitution>
    runs: each round
  # - id: browser  # when the project has a screen: one test that drives it through what the change touches
  #   type: machine
  #   command: <browser test command from the constitution>
  #   runs: before review
  #   when: [':(glob)<the paths of what a person sees>']  # and add it to the checks of every color
  # - id: mutation  # not at the start: hardening adds it, or a threshold of coverage, when the tests proved weak
  #   type: machine
  #   command: <mutation command, on what `git diff <start> HEAD` gives and nothing else>
  #   passes: <the tool's own default, looked up on its page: never one from memory, never the highest>
  #   runs: before review
  - id: review
    type: reviewer
  # - id: hidden          # only from platform up, when a color asks for it
  #   type: machine
  #   command: <how the hidden examples run, where the builder can't read them>
  - id: brief-approval
    type: person
  - id: person-review
    type: person
colors:
  green:
    checks: [seal, tests, lint, secrets]
    level: light
    limits:
      minutes_per_change: 10  # of work: without the time it waits for a person
      rounds_per_change: 1
      fix_rounds_per_gate: 1
      runs_before_review: 1
  yellow:
    checks: [seal, tests, lint, secrets, review]
    level: standard
    limits:
      minutes_per_change: 30
      rounds_per_change: 2
      fix_rounds_per_gate: 1
      runs_before_review: 2
  red:
    checks: [brief-approval, seal, tests, lint, secrets, review, person-review]
    level: standard  # its brief, like preparing and hardening, takes the deep level
    limits:
      minutes_per_change: 60
      rounds_per_change: 3
      fix_rounds_per_gate: 2
      runs_before_review: 2
zones:
  - id: <zone>
    color: <green, yellow or red>
    paths:
      - ':(glob)<pattern>'
    # gates: gdd/gates/<zone>.feature   # only once this zone has a gate of its own
# hidden: <where the hidden examples live, when a color asks for them>
context:
  - file: gdd/context/<file>
    for: [<zone or zone/Rule>]
  - mcp: <server, when a gate needs one>
    for: [<zone>]
---

# Blueprint

<Two or three sentences: what the project is, and what in it is delicate.>

```mermaid
flowchart LR
  change([A change]) --> color{Its color}
  color -->|green| green[seal, tests, lint, secrets]
  color -->|yellow| yellow[+ reviewer]
  color -->|red| red[+ brief approval, a person's review]
```

| Zone | Color | Paths | What can go wrong |
|---|---|---|---|
| <zone> | <color> | `<pathspec>` | <in one line> |

| Check | What it catches | When it runs | Colors |
|---|---|---|---|
| <check> | <in the person's words> | <every round, or once before the review> | <colors> |

Where a person approves is this project's decision. `brief-approval` and `person-review` on red are a proposal: on yellow the brief and its scenarios are still written, the person is told in a line what the change will do, and nobody waits for them. Add or remove them in `colors`, on any color: the person, or their team, can ask for `brief-approval` on yellow too. Removing one later loosens a gate, so the person approves it. A change goes through its checks in two tiers: each round, the seal, the tests of the change, lint and types and the secret scan, under two minutes together; then once, before the reviewer, what takes longer, such as one browser test of what the change touches when it touches what a person sees, under ten minutes in all. The caps of each color are a start, which the project's logs correct, and a limit that runs out stops the change. Mutation testing and a threshold of coverage are in no color at the start: hardening adds them, at their tool's own default, when the tests proved weak. A check that not every color asks judges only what the change adds or changes: if it judged the whole code, the first change of that color would answer for what the lighter ones shipped. From `platform` up, red can ask for a `hidden` check too: more examples of the same scenarios, kept where the builder's credentials can't read them, a place the front matter names in `hidden`. Anything no zone covers is never green, and it stops no change: when the orchestrator briefs the change, it judges the path against what this blueprint marks as delicate, yellow unless it does what a red zone does, and it asks only when it can't tell.

## Emergencies

<What an urgent change may skip, who opens that gate and what it may never skip. A person opens it, it's written to the log as a waiver, and the change goes through what it skipped afterwards.>

## Baseline

Measured on <date>, over <n> changes, before GDD: <time from request to release>, <stops per change>, <what slipped through>.

## What would make us simplify

<The figure that would make this team drop a check, or a question of the reviewer's, as something its logs can count: how many changes it ran on without catching anything real, or what it may cost each one.>
