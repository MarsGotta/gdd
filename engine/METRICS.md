# Metrics

`gdd metrics` derives every figure from four sources and nothing else: the logs (`gdd/log/<id>.jsonl`), the tool's journals (`gdd/log/<id>.engine.jsonl`), the blueprint and git. Run it again on the same folder and it gives the same tables, so a benchmark can recompute them from a run's project folder alone.

```sh
gdd metrics --all --format json             # one object: {"schema": "gdd-metrics/1", "tables": {...}}
gdd metrics --all --format jsonl            # one row per line, each with "schema" and "table"
gdd metrics --all --format csv --out dir    # one file per table: changes.csv, steps.csv, ...
gdd metrics --all --summary                 # what a person reads
gdd metrics empty-cart --format json        # one change
```

It needs neither git nor the blueprint to run: without git the columns that come from git are null, and without a blueprint there are no caps and no tiers. `--all` reads every log and every journal of `gdd/log/`, so the `engine` table holds every call of the layer, even one whose change never wrote a line: a program finds them all with `gdd metrics --all --format jsonl` and the rows whose `table` is `engine`, without knowing any id. There is no single journal for the whole project on purpose: every change's bookkeeping commit would carry a different tail of it, and two changes shipped by pull request would conflict on it.

The schema is `gdd-metrics/1`. A column that can't be known is `null` (empty in CSV). Lists are JSON lists, joined with `;` in CSV. Times are UTC. Minutes have one decimal unless they are whole by their source.

## changes: one row per log, the blueprint's left out

| Field | Unit | Where it comes from |
|---|---|---|
| change | id | the log's name |
| kind | `change` or `setup` | `standing-checks*` and `gates-*` logs are `setup` |
| color_planned | color | the first `brief` step |
| color | color | the last `brief` step |
| color_touched | color | the last `review` step's `color.color`, on what was touched |
| rose | bool | color_touched above color_planned |
| zones, uncovered | lists | the last `brief` step's `color` |
| scenarios | count | the brief's front matter `scenarios`, else the `@brief:<id>` tags of its sealed feature files |
| files_changed, lines_added, lines_deleted | count | `git diff --numstat <start> <head>`, head being the last gates' `head` or the change's branch; null without git |
| events, steps | count | readable lines, and those that are steps |
| minutes_work | minutes | as the counter counts them, from the first event to the last, without the waits for a person |
| minutes_waiting | minutes | each `ask` to the event after it |
| minutes_parked | minutes | always null: no event says a change was set aside |
| minutes_to_ship | minutes | the first event to the `ship`, waits included |
| builds, checks_launches | count | `build` and `checks` steps |
| rounds_used, rounds_allowed, rounds_raised | count, count, bool | the counter: launches of gdd-build after the first, or of gdd-checks in a log of standing checks, against `rounds_per_change`, and whether a waiver raised it |
| sent_back_checks_*, sent_back_review_* | count | gates that failed, reviews that said `fix`, against `fix_rounds_per_gate` |
| minutes_used, minutes_allowed, minutes_raised | minutes | against `minutes_per_change` |
| runs_before_review_* | count | gates runs that ran a check whose `runs` is `before review` |
| tokens_used, tokens_allowed, tokens_raised | tokens | every `tokens` of the log, against `tokens_per_change` when it has one |
| seconds_each_round_most, seconds_before_review_most, and their _allowed | seconds | the slowest run of each tier of checks, against its tier |
| waivers, asks, asks_answered | count | events of the log |
| gates_runs, gates_first_try | count, bool | the first `gates` step passed |
| reviews, review_first_try | count, bool | the first `review` said `pass` |
| ended | `shipped`, `stopped` or `open` | a `ship` step; or a next step stopped for a reason other than waiting for the person |
| stop_reason | code | `cap-spent`, `review-stop`, `seal-broken`, `cannot` |
| next | step | the next step the tool derives |
| tokens | tokens | the sum of the steps' `report.tokens` |
| role_seconds | seconds | the sum of the steps' `report.seconds` |
| suspect_times | count | role steps written in the same second as the step before |
| verify_errors, verify_warnings | count | `gdd verify` of the log, without git, with the journal telling the layer's own lines apart |
| invocations, refusals | count | journal lines of the change, and those that refused |
| launches, launches_not_started, launches_timed_out | count | journal lines of `gdd launch --run` |

## steps: one row per step event

change, line, ts; step; role (the report's skill) and skill_version; level; model; round; result (the report's result, the verdict of a review, pass or fail for gates); caught; seconds, tokens and tokens_read, as the report gives them; seconds_since_previous, from the event before with a time, which is how long that step took if it was written when it closed; suspect_time, true when a step that takes time (checks, build, gates, review, reading) has the same second as the step before it, the sign of a log written afterwards, unless the journal shows that each of the two was written by its own call of the layer; by_layer, true when the journal shows a call of the layer wrote that line.

## checks: one row per executed or skipped check of each gates run

Each row identifies the change, run number, log line, head, start, check and tier (`each round` or `before review`). A run gives the command, exit code, measured seconds and result (`pass` or `fail`). An omission has `result: "skipped"` and its logged reason in `why`. Its command, exit and seconds are null. A path filter can exclude a check, or a failure can stop a later tier. Old logs can only show the omissions they recorded.

## claims: one row per check the builder reported in a round

`change`, `line` and `round` identify the build. `check` is its blueprint id. `failures` counts failed runs of that check in that round, including a final failure. The last run supplies `command`, `exit` and `tail`. A claim with `failures: 0` says the builder saw no failure. An older claim can have a successful final exit and an unknown failure count. Its count is null. A build with no claims contributes no rows to this table.

## hardening: one row per recorded Harden step

`line` identifies the blueprint log entry. `change` names the originating change when there is one. For each `crack`, `expected_check` identifies what should have caught it and `caught_by` identifies the check, person or place that found it. Each row gives the `proposal`, its `action` and the recorded `result` (for example, proposed or approved). For `relax` and `retire`, `expected_check` names the check under consideration. `catches` and `cost` give the evidence for the period examined. Older entries leave missing fields null. A proposal and its later approval can describe the same defect.

## asks: one row per question to the person

change, line, ts; kind (its `of`); what; answered (an event followed it); answered_by (the kind of that event); got_what_it_asked (an `answer`, the approval of its kind, or a waiver); minutes_until_answered; minutes_open, until now, for a question still open.

## findings: one row per finding of a review or a reading, and per item left for later

change, line; source (`review` or `reading`); round; finding (its id); kind (`finding`, `reading` or `later`); what; where; went: for a review's finding, how a later `closed` ended it (`fixed`, `false`, `intended`, `minor`) or `open`; for a reading's, where it went (`brief`, `person`, `log`, `dropped`); `backlog` for what was left under `LATER:`.

## engine: one row per invocation of the tool, from the journals

change; ts; command; args (as journaled, free text as `<text>`); exit; ms; wrote_line (the log line the call wrote, if any); refused (a short code: `needs-approval`, `cap-spent`, `not-merged`, `out-of-order`, `no-report`, `no-verdict`, `not-started`, `timeout`, `hash-mismatch`, `seal-mismatch`, `wrong-commit`...); and for `launch --run`: role, model, adapter, started, seconds_to_first_output, seconds, timed_out, tokens, and input_tokens, output_tokens, cache_read_tokens, cache_write_tokens as the environment reported them.

A launch records tokens only when its environment reports them, read by an adapter chosen by the launched program (`--adapter` overrides it). A step's `tokens` is fresh input plus cache writes plus output; cache reads are kept apart in the journal, because a long session repeats them on every turn. The steps table also gives `tokens_read`, the round’s input including cache reads and writes. Codex already includes cached input in its input total. Claude reports fresh input, cache reads and cache writes separately. Output is excluded. An unavailable count or an older step gives null.

## The summary

`gdd metrics --all --summary` answers the following from the selected logs and journals.

- **What did each check catch?** One column counts failures and runs in `gates`. Another sums the failed runs builders reported in their claims. Each claim counts once, even when the gates ran more than once. Missing claim counts and whole build steps without claims are shown separately. A failed run can be a repeat of the same defect. Harden entries are grouped by the check that should have caught each defect and who or what found it. The count includes every recorded proposal or approval in that group.
- **What did it cost?** For each check, the summary adds measured gate seconds, counts untimed runs and groups skips by reason. Skips contribute neither runs nor time. For each role it sums reported seconds and tokens independently and counts the steps missing each. A partial sum covers only measured steps. If none were measured, the summary prints `unknown`. The five slowest timed checks, wall time by step and work minutes by color are also shown.
- **How much did it ask?** Each kind of question has a count, its median and longest wait, and how many are still open. For changes with a review, the first-try rate counts those whose first verdict was `pass`. Gates have their own first-try rate.
- **How much work did the layer do?** Each change has its count of journaled calls, including calls before its first log event. Project and setup calls contribute to the overall total. The summary also lists refusals, launches that never started or timed out, change outcomes, suspect times and verification results.

## What it can answer, and what it can't

- **Layer calls per change.** Read `changes.invocations` for the change id. It counts one journal entry for an explicit `next`, one for a launch that both runs and reports, and two for a manual `launch` followed by `report`. Printing the next command after a write adds zero. To see where the calls went, group that change's `engine` rows by `command`. Calls without a change id belong to the project journal. Waits in the calling tool can only be counted in its trace.
- **Cost and turns.** The role totals use the steps’ reports. The journal records calls, refusals and the model’s usage breakdown. The orchestrator's own tokens and turns are invisible to the tool: it never sees the conversation that calls it. A benchmark measures those from outside and joins them by change id.
- **No run stuck or voided by bookkeeping.** Roles that never started or timed out, and every refusal with its reason, are in `engine`; changes stopped and why in `changes`.
- **Every log passing verify.** `verify_errors` and `verify_warnings` per change; `gdd verify --all` gives the same with git as well.
- **Time and scenarios.** Minutes of work per change and per step, waits for the person apart, and the scenarios of each brief.

Not measured: how long a change sat set aside (`minutes_parked`), the time a role took when its step wasn't written as it closed (only `seconds_since_previous`, flagged by `suspect_time`), the seconds of a check nothing timed, the lines of a change without git, and anything that happened outside the log, such as a role launched by hand and never reported.
