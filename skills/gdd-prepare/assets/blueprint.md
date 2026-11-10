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
  - id: person-review
    type: person
colors:
  green:
    checks: [tests, lint]
    level: light
    back_to_person: false
  yellow:
    checks: [tests, lint, mutation, review]
    level: standard
    back_to_person: false
  red:
    checks: [tests, lint, mutation, hidden, review, person-review]
    level: deep
    back_to_person: true
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
  color -->|yellow| yellow[+ mutation, reviewer]
  color -->|red| red[+ hidden examples, a person]
```

| Zone | Color | Paths | What can go wrong |
|---|---|---|---|
| <zone> | <color> | `<pathspec>` | <in one line> |

On red, a person reviewing the change is recommended, not required: remove `person-review` from red if this project decides otherwise. Anything no zone covers counts as red until the person classifies it.
