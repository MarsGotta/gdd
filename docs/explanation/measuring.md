# Measuring what matters

What GDD measures, why it counts tokens before dollars, how it borrows DORA's delivery metrics,
and how a number becomes a decision. Read it before you judge whether GDD is paying off in your
project.

## Before you start

[GDD in one page](method.md) gives the principles; the last one is the subject of this page. The
reports described here are `gdd report`, `gdd report --time` and `gdd report --dora`, checked by
[`features/report.feature`](../../features/report.feature),
[`features/flow-time.feature`](../../features/flow-time.feature) and
[`features/delivery-report.feature`](../../features/delivery-report.feature).

## Outcomes, not output

The eighth principle of GDD says: *measure outcomes, not output*. Output is what the agents
produce: lines of code, files, calls. Outcomes are what you get: changes that reach production,
stay there, and needed little of your time.

Output is cheap to count and quick to inflate. An agent asked for more code writes more code.
When a measure becomes a target, it stops being a good measure (Goodhart's law); DORA lists
"setting metrics as a goal" first among the
[pitfalls of its own metrics](https://dora.dev/guides/dora-metrics/#common-pitfalls). So GDD
never sets a target on lines of code, and treats them at most as a sign of bloat.

## Tokens first, dollars second

The cost of a change is the tokens its agents used. Every agent call adds one line to
`.gdd/log.jsonl`, read from the agent's own JSON output, and `gdd report` adds them up:

```text
role -> calls · tokens in · cache · out · cost USD · seconds
```

The columns keep three kinds of tokens apart:

- **tokens in**: input the model read fresh, not from its cache;
- **cache**: input it read from the cache, much cheaper and growing with every turn of a long
  session;
- **out**: what it wrote.

Adding the three into one number hides what changed. A long session can show tens of millions of
cached tokens and a modest number of new ones; what you compare between two ways of working is
the new input and the output.

Dollars come second because they are not a measurement everywhere. Claude Code reports a cost
that is a client-side estimate. Codex reports none; a price table in `gdd/models.toml` lets the
log estimate it. With a subscription, the cost per call is notional. Tokens are what every agent
reports, so tokens are what compares.

`gdd report --by change,model,role` groups the calls any way you need: by change, phase, model,
role or agent. What one change spent also lives in its record, which only gdd writes, and in its
merge commit.

## Where the time goes

A change can be cheap and still slow. `gdd report --time` follows each change from its first
line in the log to its last, its **flow time**, and splits it:

- **active**: agent calls by role and model, gate runs by command, release steps;
- **waiting**: at the sitting, before the change starts, parked until relaunched, for you at G4;
- **between steps**: what is left.

**Flow efficiency** is active time over flow time, a measure from the Flow Framework. A change
that took six hours with forty minutes of work did not need faster agents: it waited. The report
names the biggest waits, so you know which step to polish first. The Board shows the same in a
change's details and in a report view.

## DORA's five

[DORA's research](https://dora.dev/guides/dora-metrics/) names five software delivery metrics, in
two groups:

| Group | Metric | DORA's definition, in short |
|---|---|---|
| Throughput | Change lead time | From committed to deployed in production |
| Throughput | Deployment frequency | Deployments per period, or the time between them |
| Throughput | Failed deployment recovery time | Time to recover from a deployment that needs immediate intervention |
| Instability | Change fail rate | Share of deployments that need immediate intervention |
| Instability | Deployment rework rate | Share of deployments that are unplanned, caused by an incident in production |

`gdd report --dora` computes them from the log and the records, with these choices:

- A **deployment** is a promotion: the `promote` step of the release, passed or simulated.
- A deployment **failed** when its observation failed or it was rolled back.
- **Change lead time** runs from the build's start to the promotion.

DORA's metrics are why GDD measures delivery, not activity. Its
[2025 report](https://dora.dev/research/2025/dora-report/) adds a warning that fits agents well:
AI is "an amplifier, magnifying an organization's existing strengths and weaknesses". Faster
agents in a weak process ship weak software faster.

## GDD's own signals

DORA does not see inside a change. GDD adds the signals its own gates produce, printed after
DORA's five under a heading that says what they are: *GDD signals (signs to read, never
targets)*.

| Signal | What it suggests when it rises |
|---|---|
| Polish at G4 | Changes look done before they are: the standards or the package miss something |
| Parks per change | Changes are too big, or their packages lack context |
| Reviewer KO | The Builder misses what the Reviewer sees: a lesson for Learn |
| Reviews to pass | Findings come in drips instead of one pass |
| Mutation testing red in G3 | Tests that do not check what they seem to |
| Human stops per change | You are asked more than the method promises |
| Tokens out per change | Cost per change, by its median |

None of them is a goal. A team that pushed its polish rate to zero would do it by not looking.
Each one is a question to ask in Learn.

## The kill rule

Every MVP in GDD gets a **kill rule** in Discover: what would have to happen to abandon it. It is
written before the work starts, while nobody is invested yet, and it is written in numbers that
the reports can show.

gdd has one for itself, in its [roadmap](../../ROADMAP.md): if gdd spends more tokens or asks for
more human stops than the tool it replaces, on the same kind of change, stop and review before
adding more. The two numbers in that sentence are a column of `gdd report` and a line of
`gdd report --dora`.

## From numbers to Learn

Learn, phase 5, is where the numbers turn into changes. A repeated polish note becomes a line of
`docs/standards.md`; a class of Reviewer finding that comes back becomes a lint or a test; a step
that always waits becomes the next thing to fix. `gdd learn` lists the repeats; the reports say
whether the fix worked.

## An example: one Builder session or two

By default the Builder runs test mode and build mode in separate sessions: build mode starts
fresh and reads what test mode wrote. A reasonable objection is that the handoff loses context
and spends tokens reloading it. gdd settled the question with an experiment on its own changes.

The setup: ten closed changes replayed from their base commit, each run twice with each setup
and the model pinned, graded by their own approved and hidden scenarios. Setup A was the default,
two sessions. Setup B was one Builder session that wrote the steps first and then continued into
build mode, with Path Guard proving the steps were not edited after their first run.

The result, over 40 runs:

| | A: two sessions | B: one session |
|---|---|---|
| Runs that passed every gate | 14 of 20 | 13 of 20 |
| New tokens | 6.0M | 4.7M, about 21% fewer |

B also took about 7% less time. Quality was within the noise: four runs of each setup failed on
the sandbox (mutation testing timed out with two changes at once), and most of the other
failures, two for A and three or four for B, were mutants that no test caught. The saving was
real but modest.

So the default stayed two sessions, and one session became a choice:
`[session] builder = "shared"` in `gdd/models.toml`, for Claude Code today, where the agent sheet
says how to resume a session. Path Guard, mutation testing and the Reviewer stay as they are.

The point is the method, not the result. A question that sounds like taste (one session or two?)
became a measurement with a fixed grading and a rule set before the runs: keep one session only
if quality holds.

## See also

- [Measure the work](../how-to/measure.md): the report commands, step by step.
- [Command reference](../reference/cli.md#gdd-report): every option of `gdd report`.
- [Few stops for the human](human-stops.md): where polish and human stops come from.
- [Design decisions](design-decisions.md#separate-test-and-build-modes): why two modes at all.
