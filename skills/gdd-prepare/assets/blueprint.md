---
contract: gdd/1
guarantee: <rule, platform or engine>
levels:  # one block for each agents' environment the project is worked from
  <environment, by the name of its command>:
    launch: <how a role gets its model there, by its tool for launching agents or by a new session of its command line>
    light: <model for little reasoning, cheap and fast, by the name that environment launches it with>
    standard: <model for normal reasoning>
    deep: <model for the most reasoning>
    # reviewer:            # the reviewer's models, when they are others, from another model family where there is one
    #   launch: <how, when it runs in another environment>
    #   light: <model>
    #   standard: <model>
    #   deep: <model>
limits:
  fix_rounds_per_gate: 2
  rounds_per_change: 6
  minutes_per_change: 120  # without the time it waits for a person
  lines_per_change: 400  # the builder's: git diff --shortstat <start> HEAD
  tokens_per_change: <n, where the environment reports them>
gates:  # the gates of the project, the same for every change, each one specified in its feature
  - id: <gate>
    spec: gdd/gates/<gate>.feature
    checks: [<check id>]
checks:
  - id: seal
    type: machine
    command: "git diff --exit-code <start> HEAD -- gdd features AGENTS.md CLAUDE.md .agents .claude <sealed paths> ':(exclude)gdd/log' ':(exclude)gdd/backlog.md'"
  - id: tests
    type: machine
    command: <test command from the constitution>
    breaks: <the bad case to plant to see it fail>
    cases: each change  # its cases are written for every change with a brief
    # passes: <what counts as passing, when exit 0 isn't all of it>
    # when: [':(glob)<pattern>']  # it applies only to a change that touches these paths
  - id: lint
    type: machine
    command: <lint command from the constitution>
  - id: secrets
    type: machine
    command: <secret scan command from the constitution>
  - id: mutation
    type: machine
    command: <mutation command, on what `git diff <start> HEAD` gives and nothing else, where the stack has a tool for it>
    # passes: <the score it closes at: the tool's default or what the code reaches, never the highest>
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
  yellow:
    checks: [brief-approval, seal, tests, lint, secrets, review]
    level: standard
  red:
    checks: [brief-approval, seal, tests, lint, secrets, mutation, review, person-review]
    level: deep
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
  color -->|yellow| yellow[+ brief approval, reviewer]
  color -->|red| red[+ mutation, a person's review]
```

| Zone | Color | Paths | What can go wrong |
|---|---|---|---|
| <zone> | <color> | `<pathspec>` | <in one line> |

Where a person approves is this project's decision. `brief-approval` on yellow and red and `person-review` on red are a proposal: add or remove them in `colors`, on any color. Removing one later loosens a gate, so the person approves it. Take `mutation` out of red if the stack has no mature tool for it, and add it to yellow if it has one and the team wants it. A check that not every color asks, like `mutation`, judges only what the change adds or changes: if it judged the whole code, the first change of that color would answer for what the lighter ones shipped. From `platform` up, red can ask for a `hidden` check too: more examples of the same scenarios, kept where the builder's credentials can't read them, a place the front matter names in `hidden`. Anything no zone covers is never green, and it stops no change: when the orchestrator briefs the change, it judges the path against what this blueprint marks as delicate, yellow unless it does what a red zone does, and it asks only when it can't tell.

## Emergencies

<What an urgent change may skip, who opens that gate and what it may never skip. A person opens it, it's written to the log as a waiver, and the change goes through what it skipped afterwards.>

## Baseline

Measured on <date>, over <n> changes, before GDD: <time from request to release>, <stops per change>, <what slipped through>.

## What would make us simplify

<The figure that would make this team drop a check, or a question of the reviewer's, as something its logs can count: how many changes it ran on without catching anything real, or what it may cost each one.>
