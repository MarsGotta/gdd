---
contract: gdd/1
levels:
  light: <model for little reasoning, cheap and fast>
  standard: <model for normal reasoning>
  deep: <model for the most reasoning>
limits:
  fix_rounds_per_gate: 2
  rounds_per_change: <n>
  minutes_per_change: <n>
  lines_per_change: <n, counted with git diff --shortstat>
checks:
  - id: tests
    type: machine
    command: <test command from the constitution>
  - id: lint
    type: machine
    command: <lint command from the constitution>
  - id: mutation
    type: machine
    command: <mutation command, on the files the change touched>
  - id: hidden
    type: machine
    command: <command that runs the hidden examples>
  - id: review
    type: reviewer
  - id: brief-approval
    type: person
  - id: person-review
    type: person
colors:
  green:
    checks: [tests, lint]
    level: light
  yellow:
    checks: [brief-approval, tests, lint, mutation, review]
    level: standard
  red:
    checks: [brief-approval, tests, lint, mutation, hidden, review, person-review]
    level: deep
zones:
  - id: <zone>
    color: <green, yellow or red>
    paths:
      - ':(glob)<pattern>'
    gates: gdd/gates/<zone>.feature
context:
  - file: gdd/context/<file>
    for: [<zone>]
---

# Blueprint

<Two or three sentences: what the project is, and what in it is delicate.>

```mermaid
flowchart LR
  change([A change]) --> color{Its color}
  color -->|green| green[tests, lint]
  color -->|yellow| yellow[+ brief approval, mutation, reviewer]
  color -->|red| red[+ hidden examples, a person's review]
```

| Zone | Color | Paths | What can go wrong |
|---|---|---|---|
| <zone> | <color> | `<pathspec>` | <in one line> |

Where a person approves is this project's decision. `brief-approval` on yellow and red and `person-review` on red are a proposal: add or remove them in `colors`, on any color. Removing one later loosens a gate, so the person approves it. Anything no zone covers counts as red until the person classifies it.
