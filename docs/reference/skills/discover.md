# gdd-discover

The discover skill is Phase 1 of GDD, a conversation with you. Starting from the Project
Brief, the Orchestrator explores one topic, brings you options to decide, runs a spike when a
question blocks a decision, and proposes the next changes in order. It writes no product code.

Run it after Setup when the product first needs an architecture or a map of its sensitive
areas, and whenever the next step is bigger than one small change.

## Before you start

- G0 is open: [`gdd-setup`](setup.md) is done and [`gdd status`](../cli/status.md) says
  `G0: ok`.

## Call it

With a topic, in Claude Code and OpenCode:

```text
/gdd-discover how the notes are stored, and whether they sync between machines
```

In Codex:

```text
$gdd-discover how the notes are stored, and whether they sync between machines
```

Without a topic, `/gdd-discover` takes the next piece of value for the product.

## What it asks you

It does not repeat the Setup questions: it builds on `docs/brief.md`. For each important
question of the topic (design, data, privacy, risks, market) it brings two or three options,
each with its pros and cons, and its recommendation. It searches the web when the question
needs it. When the Brief has no minimum viable product (MVP) or kill rule yet, it proposes the
smallest MVP with value and what would have to happen to abandon it.

Few questions, grouped, with the recommendation first. Every message ends with one next step.

## What it does

1. **Reads the Brief** and works from your earlier answers.
2. **Explores** the topic with you, option by option.
3. **Runs a spike only when a question blocks a decision.** A spike is a change of type Spike,
   with a fixed budget, that answers one question and leaves an architecture decision record
   (ADR). It creates it with
   [`gdd new-change --type Spike`](../cli/new-change.md). When the question has 2 to 4
   options, it asks for one variant per option (`--option SQLite --option Postgres`); with no
   options to tell apart, for several variants of the same brief (`--variants 2`). You compare
   the variants at the Release Gate (G4) and pick one. See
   [Explore with a spike](../../how-to/spikes.md).
4. **Records costly decisions** as ADRs in `docs/adr/NNNN-title.md`: the context, the
   decision, the alternatives and the consequences.
5. **Proposes the next changes**, in order, each one small and valuable.
6. **Maps the red zones, at the end.** Only now are there real folders to map: it turns the
   sensitive areas of the Brief into paths in `gdd/zones.toml`, red, and green where it is
   obvious. Between two colors it picks the higher one. It does not ask for your approval yet:
   you approve the red zones at the sitting of the first change.
7. **Saves the conversation** in `docs/discover/YYYY-MM-DD-<topic>.md`, and updates
   `docs/brief.md` when something changed.
8. **Counts the conversation** with [`gdd session --phase 1-discover`](../cli/session.md).

## What you decide

- The option for each question, or that it needs a spike first.
- The MVP and the kill rule, when the Brief has none.
- The order of the next changes.
- At G4 of a spike with variants, which variant its ADR keeps.

The red zones it maps are not decided here: they wait for your approval at the next sitting.

## What it leaves behind

| What | Where |
|---|---|
| The record of the conversation | `docs/discover/YYYY-MM-DD-<topic>.md` |
| Decisions that are costly to change | `docs/adr/NNNN-title.md` |
| An updated Brief, when something changed | `docs/brief.md` |
| The red zones, not yet approved | `gdd/zones.toml` |
| A spike, when one was needed | `.gdd/changes/<id>/` and its branch `change/<id>` |

[`gdd status`](../cli/status.md) shows the red zones as pending until you approve them.

## What comes next

The skill ends by proposing [`gdd-change`](change.md) for the first change of the order it
proposed. Its sitting shows the red zones with the scenarios, for one OK.

## See also

- [Map and approve red zones](../../how-to/red-zones.md): how a path gets its color.
- [Explore with a spike](../../how-to/spikes.md): budgets, variants and the ADR.
- [GDD in one page](../../explanation/method.md#phases-and-gates): where Discover sits among the
  phases.
