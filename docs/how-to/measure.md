# Measure the work

Find out what your changes cost in tokens, where their time goes, how delivery is doing by
DORA's measures, and what Learn should harden next, from the log gdd keeps.

## Before you start

- A GDD project with some changes run: every agent call, gate and human decision appends one
  line to `.gdd/log.jsonl`, and each change keeps its record in `.gdd/changes/<id>/`.
- Read [Measuring what matters](../explanation/measuring.md) for why tokens, not lines of code,
  and why the GDD signals are signs to read, never targets.

The examples on this page are real output from gdd's own repository, which is built with GDD:
50 changes in about a week.

## 1. Count the tokens

```bash
bin/gdd report
```

You should see one line per role, then the total:

```text
role -> calls · tokens in · cache · out · cost USD · seconds
builder-build -> 178 · 23,521,360 · 937,297,814 · 8,591,698 · 526.6018 · 85112
builder-test -> 63 · 2,708,102 · 28,256,111 · 328,557 · 16.9410 · 6604
orchestrator -> 10 · 535,311 · 7,194,620 · 123,555 · 8.1913 · 1336
reviewer -> 124 · 8,677,291 · 51,913,984 · 345,867 · 0.0000 · 12263
TOTAL -> in 35,442,064 · cache 1,024,662,529 · out 9,389,677 · 551.7341 USD
Human stops logged: 201
```

Read the columns in this order:

1. **tokens in** and **out**: new input tokens and output tokens. These are what compare
   between changes, models and roles.
2. **cache**: input tokens read from the agent's prompt cache. They are many (here about 29 for
   every new input token) and much cheaper, so do not add them to the new ones.
3. **cost USD**: an estimate at API prices, never a bill. Claude Code reports its own
   client-side estimate; OpenCode reports a cost; Codex reports none, so the Reviewer above
   shows 0 until you fill in `[prices]` (see
   [Choose agents and models](agents-and-models.md#6-fill-in-prices-for-estimates)). With a
   subscription the cost is notional: compare tokens.

Group and filter:

```bash
bin/gdd report --by agent,model
bin/gdd report --by role --change CHANGE
bin/gdd report --by model --json
```

`--by` takes any of the log's fields separated by commas (`change`, `phase`, `model`, `role`,
`agent`...); the default is `role`. With `--by agent,model`:

```text
agent · model -> calls · tokens in · cache · out · cost USD · seconds
claude · opus -> 140 · 21,382,030 · 896,621,430 · 8,056,377 · 512.7214 · 78893
claude · sonnet -> 111 · 5,382,743 · 76,127,115 · 987,433 · 39.0127 · 14160
codex · gpt-6-astra -> 124 · 8,677,291 · 51,913,984 · 345,867 · 0.0000 · 12263
TOTAL -> in 35,442,064 · cache 1,024,662,529 · out 9,389,677 · 551.7341 USD
Human stops logged: 201
```

`--json` prints the same groups as a list, with `tokens_in`, `tokens_cache`, `tokens_out`,
`cost_usd` and `duration_s`.

Conversations with your agent outside the recipe (the Setup, Discover) are not in the log until
you add them. For Claude Code:

```bash
bin/gdd session --phase 1-discover
```

## 2. See where the time goes

```bash
bin/gdd report --time --change ratchet
```

```text
ratchet · A ratchet for known debt: a change cannot add to it, and what it pays stays paid · closed
  Flow time 1h 18m · active 1h 07m (86%) · waiting 11m · between steps 0s
  Agents: builder-build · opus 35m, reviewer · gpt-6-astra 5m, builder-test · sonnet 1m
  Gates: mutation 15m, acceptance_full 8m, unit 13s, security 4s, acceptance 3s, dependencies 2s, secrets 1s, lint 1s, lockfile 0s, architecture 0s, path-guard 0s
  Release: demo 2m, build 9s
  Waiting: at the sitting 9m, for the human at G4 2m, to start 8s
```

- **Flow time** runs from the change's first line in the log to its last.
- **Active** is agent calls, gate commands and release steps; the percentage is flow
  efficiency (active divided by flow time).
- **Waiting** is time for a person: at the sitting, at G4, parked until relaunched, in the
  merge queue, before the start.
- **Between steps** is the rest: gdd's own time between steps, or a gap in the log.

Without `--change`, the report lists every change and ends with the project's totals and its
biggest waits:

```text
...
Project: 50 change(s) · flow time 106h 27m · active 47h 45m (45%) · waiting 59h 02m · between steps 22m
Biggest waits: for the human at G4 32h 49m, at the sitting 18h 12m, to start 3h 57m
```

Here the people, not the agents, were the slowest step: a third of the flow time was changes
waiting at G4. Grouping sittings and answering G4 sooner would save more than a faster model.
`--since YYYY-MM-DD` keeps only the changes created on or after that day (UTC); `--json` gives
one object per change.

## 3. Check delivery with DORA's measures

```bash
bin/gdd report --dora
```

```text
Delivery · 48 deployment(s) from 2026-10-25 to 2026-10-30 · 48 simulated
Throughput
  Change lead time: median 1h 17m, from the build's start to the promotion
  Deployment frequency: 8.0 per day · median 1h 29m between deployments
  Failed deployment recovery time: none failed
Instability
  Change fail rate: 0% (0 of 48)
  Deployment rework rate: 2% (1 of 48 are Hotfixes)
GDD signals (signs to read, never targets)
  Polish at G4: 14% (8 of 56 decisions)
  Parks: 0.7 per change (37 in 50)
  Reviewer KO: 48% (59 of 124 reviews)
  Reviews to pass: median 2 per change · first review OK in 12 of 48 changes
  Mutation testing red in G3: 11 of 70 runs
  Human stops: 2.1 per change (105 in 50)
  Tokens out: median 190,562 per change
```

The first block is DORA's five delivery metrics: two of throughput, three of instability. The
deployments here are simulated (this repository has no real production), which the first line
says. The second block is GDD's own signals:

| Signal | What a high value suggests |
|---|---|
| Polish at G4 | Done is not good: work passes the gates but is unfinished. Feed `docs/standards.md`. |
| Parks | Changes too big, or gate commands that fail for the machine, not the code |
| Reviewer KO, reviews to pass | The Builder repeats a class of bug: make it a rule (step 5) |
| Mutation testing red | Tests that run the code without checking it |
| Human stops | Too many sittings: group them |
| Tokens out | Cost per change, to compare across months or models |

`Human stops` here counts a sitting once and each G4 answer once. The `Human stops logged`
line of `gdd report` counts every human decision in the log, so three approvals at one sitting
count three times there.

Compare periods with `--since`:

```bash
bin/gdd report --dora --since 2026-10-29
```

`--json` gives one object with fields such as `lead_time_s`, `change_fail_rate`,
`polish_rate` and `reviewer_ko_rate`, for a dashboard of your own.

## 4. Apply your kill rule

Discover writes a kill rule in `docs/brief.md`: the result that would make you stop and review
before going on. gdd's own was: if gdd spends more tokens or asks for more human stops than the
previous tool on the same kind of change, stop and review before adding more.

1. Write the rule with numbers gdd reports: tokens out per change, human stops per change, flow
   time.
2. At each Learn, run `bin/gdd report --dora --since DAY` for the period and compare.
3. When the rule fires, stop adding work and open a Discover instead.

## 5. Run Learn

Learn has no gate: it hardens the project. `gdd learn` runs the Learn tools of `[learn]` in
`gdd/commands.toml` (warnings only), lists the polish notes people gave at G4 and the classes
of Reviewer findings that repeat across changes.

```bash
bin/gdd learn
```

In a small Python project, you should see something like:

```text
duplication: nothing found
  ...
  All it said: .gdd/learn/duplication.txt
unused: found something (exit code 3)
  src/shop/cart.py:4: unused function 'total' (60% confidence)
  src/shop/cart.py:9: unused function 'unused_helper' (60% confidence)
  ...
  All it said: .gdd/learn/unused.txt

Polish notes at G4 (0)

Reviewer findings that repeat: none
Reviews whose findings name no class: none
```

It always exits with 0. Turn what repeats into something that runs every time:

- a polish note that comes back becomes a line of `docs/standards.md`, which the Builder and
  the Reviewer read;
- a class of Reviewer finding in the KO reviews of two changes or more becomes a lint rule, a
  check or a line of `docs/standards.md`;
- a tool's warning that matters becomes a gate command in `[g2]` or `[g3]`.

Read the tools' findings with judgement: vulture, for example, flags public functions no code
in the project calls. Without a `[learn]` section, `gdd learn` says how to add one from the
stack sheets. `--since YYYY-MM-DD` keeps only the notes and reviews of recent changes.

## Next steps

- [Tune the gates](gates-and-commands.md): turn what Learn found into gate commands.
- [Choose agents and models](agents-and-models.md): act on what the token report says.
- [Measuring what matters](../explanation/measuring.md): DORA, GDD's signals and the kill rule.
