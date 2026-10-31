# Explore with a spike

Use a spike when a question blocks a decision, such as "where do the notes live?". The Builder
writes throwaway code to find out, within a fixed budget, so you get the answer before you build
anything to keep. Only the decision reaches main, in an architecture decision record (ADR). If
the question has several options, you can build one variant per option and pick one.

## Before you start

- A project whose Setup is done: the Setup Gate (G0) is open. A spike is the only change that
  can be built before the red zones are approved (see [Map and approve red
  zones](red-zones.md)).
- A question. Discover is where spikes come from: `/gdd-discover` proposes one when a question
  blocks a decision. See [GDD in one page](../explanation/method.md).

The examples use the project `notes`.

## What a spike gets, and what it skips

A Spike runs a recipe of its own. Because its code never merges, it skips what guards code on
its way to main:

| A Spike... | Because |
|---|---|
| has no test mode, no mutation testing, no Reviewer, no G3, no merge and no pre | only its ADR reaches main |
| is built by the Builder in build mode, with `mode=spike` | it writes code to answer a question, not to keep |
| passes G2 with Path Guard and an ADR: a new or changed `docs/adr/*.md` with a `## Decision` line | the project's G2 commands do not run on throwaway code |
| always waits for a person at the Release Gate (G4) | you decide what the ADR says |
| has a budget, `[spike] max_usd` | a fixed cost, the way Discover asks |
| needs no plan, even when red | it keeps the red Builder, and you approve the ADR at G4 |

## 1. Set the budget

`gdd init` writes the budget in `gdd/rules.toml`:

```toml
[spike]
max_usd = 5   # a spike parks when its agent calls cost this much (USD)
```

The budget is in US dollars (USD). If 5 USD does not fit your spikes, change it on main (see
[Configuration](../reference/configuration.md#spike)). It must be a positive number: `0` is
refused with `spike.max_usd must be a positive number of USD, not 0`.

gdd keeps what each change spent in its record, which only gdd writes: after every agent call
it adds the cost the agent reported to `spent_usd` and the tokens to `spent_tokens`, in
`.gdd/changes/<id>/change.json`. The budget reads it from there, not from the log, so a line an
agent adds to the log cannot lift it. The budget is checked between calls: a call is never cut
in the middle, so a spike can end a little over it.

> [!NOTE]
> The budget counts what agents report. Codex reports no cost: its calls count as 0 USD unless
> `[prices.<model>]` in `gdd/models.toml` gives the price of its model. With a subscription the
> cost is notional, so compare tokens too (see [Measure the work](measure.md)).

## 2. Ask for the spike

To start a spike, describe the question to the Orchestrator and say it is a spike:

```text
/gdd-change a spike: how should notes sync between laptops?
```

It creates a change of type Spike, writes the question and what would answer it in the
Builder's package, and tells you the classification. Behind it, gdd printed for `notes-sync`:

```text
{
  "change": "notes-sync",
  "color": "yellow",
  "lane": "full",
  "reasons": [
    "docs/adr/0003-sync.md -> green (Documentation)",
    "src/sync/** -> yellow (no zone: default color)"
  ],
  "g1": false,
  "features": [],
  "tests_first": false,
  "mutation": false,
  "reviewer": false,
  "review": "quick",
  "g4": true,
  "human_plan": false,
  "type": "Spike",
  ...
```

A spike has no scenarios and no plan to approve, so the Orchestrator can launch it right away.

## 3. Let it run

The Builder works on the branch `change/notes-sync`, in its own worktree, and writes the
decision in a new file under `docs/adr/`, with a `## Decision` section. When G2 passes, the
launch says which ADR files OK would bring to main and pauses at G4. Nothing reaches main yet.

A spike parks instead when:

- it leaves no ADR: G2 fails with
  `a Spike leaves its decision in a new file under docs/adr/ with a ## Decision section`, and
  after the retries the change parks with `G2 still red`;
- it reaches its budget: `parked: the spike spent 0.02 USD of its budget of 0.02 USD` (here
  with `max_usd = 0.02`).

See [Get a parked change going again](parked-change.md) for what to do next.

## 4. Answer its Release Gate

The Orchestrator brings the spike back at G4 with its ADR. Read the decision, then answer in
words, as at any G4:

```text
OK: plain files and git, no server to run.
```

| Your answer | What happens |
|---|---|
| OK | Only the ADR files of the branch reach main, in one commit `gdd(notes-sync): Spike ADR · How notes sync between laptops`. The code stays on `change/notes-sync`, which gdd keeps. |
| KO, and why | Main stays as it was. |
| Polish, and what to polish | One more Builder round with your words, with no retry or replan, then G2. Still red, the change parks. |

There is no demo, no promotion and no changelog line: a spike delivers a decision, not a
feature. The real work comes after, as a new change built with all its gates.

## 5. Try several options at once with variants

If the question has 2 to 4 options, ask for one variant per option and compare them on one
page (set-based design: explore a set of alternatives, then narrow it down):

```text
/gdd-change a spike: where do the notes live? One variant for SQLite and one for plain files.
```

If there are no options to tell apart, ask for a number of variants of the same brief instead. The
record lists the variants, each with its branch:

```json
"variants": [
  {
    "option": "SQLite",
    "branch": "change/notes-storage-v1"
  },
  {
    "option": "Plain files",
    "branch": "change/notes-storage-v2"
  }
]
```

Read the diagram top to bottom: one launch builds every variant, and you pick at G4.

```mermaid
flowchart TD
    L[Launch notes-storage] --> V1[Variant 1 · SQLite<br/>branch change/notes-storage-v1]
    V1 --> V2[Variant 2 · Plain files<br/>branch change/notes-storage-v2]
    V2 --> P[One page with every variant]
    P --> H{Your answer at G4}:::human
    H -- "OK, variant 2" --> M[Only the ADR of variant 2 reaches main,<br/>with a Variants section]
    H -- KO --> N[Main untouched]
    classDef human fill:#e6e0f8,stroke:#6a4fc0,color:#213
```

What a launch with variants does:

- It builds the variants one after another, each by its own Builder (`mode=spike variant=<n>`),
  on its own branch `change/<id>-v<n>`, with the gate of a spike.
- Each variant has its own budget: `[spike] max_usd` counts what that variant's calls spent
  (`usd` and `tokens` of each variant in the record), and it is the variant that parks. Two
  variants can spend up to twice the budget.
- A variant that fails its gate or reaches its budget does not stop the others. When none
  passes, the spike parks with `no variant passed its gate`.

gdd refuses a spike with 5 variants (`error: a spike has 2 to 4 variants, not 5`), options and
a number of variants together, and variants on a change that is not a Spike.

## 6. Compare the variants and pick one

When the last variant is built, the spike waits at G4. Its page, in the Decision Inbox the
Orchestrator brings and on the sitting screen (`gdd sitting notes-storage`), has one line per
variant: whether it is ready or parked and why, its ADR files, the first line of its decision,
how many files it changed, its minutes and its cost. The lines look like this (illustrative,
not from a run):

```text
## Variants
- Variant 1 · SQLite · ready · ADR: docs/adr/0002-storage.md · decision: SQLite, one file per notebook. · 4 files · 6.1 min · 0.84 USD
- Variant 2 · Plain files · ready · ADR: docs/adr/0002-storage.md · decision: One Markdown file per note. · 3 files · 4.8 min · 0.52 USD
```

You compare and pick; no agent judges the variants. Name the variant in your answer:

```text
OK, variant 2: plain files keep working with git.
```

At the sitting, `o` (OK) and `p` (polish) ask `Which variant? Its number:`. Only the ADR files
of variant 2 reach main, in the commit `gdd(notes-storage): Spike ADR · Where the notes live`,
and gdd adds a `## Variants` section to the ADR that names the choice and every variant not
chosen with its branch, for example:

```text
## Variants
Chosen: variant 2 (Plain files), branch change/notes-storage-v2. Plain files keep working with git
Variant 1 (SQLite): not chosen, branch change/notes-storage-v1
```

The code of every variant stays on its branch. An OK without a variant is refused with
`pick a variant: gdd resume notes-storage --g4 ok --variant 1|2`, and a variant that did not
pass its gate cannot be picked. Polish with a variant builds that variant again with your
words; KO rejects them all.

## Under the hood

The Orchestrator creates the spike with
[`gdd new-change --type Spike`](../reference/cli/new-change.md), adding `--option` once per
option or `--variants N` for N variants of the same brief, then runs
[`gdd launch`](../reference/cli/launch.md). Your answer becomes
[`gdd resume CHANGE --g4 ok|ko|polish`](../reference/cli/resume.md), with `--variant N` to pick
a variant and `--note` for your words.

## Next steps

- [Review a change at G4](release-gate.md): the G4 page, OK, KO and polish.
- [Measure the work](measure.md): what spikes and changes cost, in tokens and USD.
- [Describe a change to your agent](run-a-change.md): build the decision of the ADR as a real
  change.
- [The discover skill](../reference/skills/discover.md): where spikes come from.
