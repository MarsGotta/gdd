---
summary: Opens the sitting of a change: approve it before Build, or answer its G4.
runs: you
---

## Who runs it

You, in a terminal, when your agent says a change waits for you. The screen never opens for an
agent that gdd started.

## Examples

Before Build, the sitting shows the change's scenarios, the red zones and, for a red change,
the plan, for you to approve or send back (G1 KO). At G4 it shows the result, for you to answer
OK, KO or polish. Nothing is written before you confirm.

```bash
gdd sitting export-csv
```

When the scenarios are already approved and nothing else waits, it says so and offers no
approval:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
┌ gdd · Sitting · shop ────────────────────────────────────────────────────────────────────────────┐
│export-csv · Export the catalog as CSV · yellow                                                   │
│Nothing waits for you in this sitting: G1 ok on export-csv.                                       │
│Scenarios                                                                                         │
│features/export-csv.feature                                                                       │
│Feature: Export the catalog as CSV                                                                │
│  Scenario: Every product becomes one line                                                        │
│    Given a catalog with 2 products                                                               │
│    When I export the catalog                                                                     │
│    Then the file has 3 lines                                                                     │
│Red zones: approved                                                                               │
│Plan: not needed for a yellow change                                                              │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Without a terminal, it says how to record the sitting instead, and exits with 2:

```text
error: gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok cart-total --by human (and gdd gate zones ok --by human, gdd gate plan ok cart-total --by human for a red change)
```

The [tutorial](../../getting-started/tutorial.md) shows a real sitting.

## See also

- [Screens](../screens.md#the-sitting): the sitting and the G4 sitting, with their keys.
- [Write and approve scenarios](../../how-to/write-scenarios.md): what to look for at G1.
- [Answer the Release Gate](../../how-to/release-gate.md): what to look for at G4.
