# gdd-report

The report skill tells you what the agent calls cost, in tokens and money, grouped the way
you ask, and sums it up in a few lines. Run it when you want to know where the spend goes, for
example before you change a role to a cheaper model.

## Call it

In Claude Code and OpenCode:

```text
/gdd-report
```

In Codex:

```text
$gdd-report
```

To group another way, say how, with any of `change`, `phase`, `model`, `role` and `agent`:

```text
/gdd-report by phase and model
```

## What it asks you

Nothing.

## What it does

1. Counts this conversation with [`gdd session --phase <phase>`](../cli/session.md), for the
   phase the project is in. It adds the requests of your interactive Claude Code sessions to
   the log.
2. Runs [`gdd report --by change,role,model`](../cli/report.md), or the grouping you asked for.
3. Sums it up in a few lines: the total, the role and the model that spend the most, the
   retries and the human stops.

An agent that does not report a price shows a cost of 0. The skill says so, and reminds you
that with a subscription the cost is notional: tokens are what compares. Fill `[prices]` in
`gdd/models.toml` to get an estimate; see
[Choose agents and models](../../how-to/agents-and-models.md#6-fill-in-prices-for-estimates).

## What you decide

Nothing. Act on the numbers with [`gdd-learn`](learn.md) or by changing models in
`gdd/models.toml`.

## What it leaves behind

The lines `gdd session` adds to `.gdd/log.jsonl`, one per model of your conversation.

## See also

- [Measure tokens, time and delivery](../../how-to/measure.md): the same numbers, plus time and
  the delivery measures of DORA (DevOps Research and Assessment).
- [`gdd report`](../cli/report.md): every grouping and flag.
