# Evidence for relaxing or retiring a check

Read `gdd metrics --all --summary` for the check you are considering. Choose the changes or dates the proposal covers. Give the person one proposal with what would still reject the bad case.

In the harden step, use `--action relax` or `--action retire` and put the check’s id in `--expected-check`. If there was no defect, use `--caught-by none`. Include both pieces of evidence:

- In `--catches`, give the period, how often the check ran, what it found in gates and what builders reported. Say which counts are missing. A failure can be another run of the same defect.
- In `--cost`, give the measured time, tokens or wait over that period and say what went unmeasured.

If their builders left no counts, say so. The gate results tell you what happened after the builders finished.

`--catches "<period>, <gate runs and failures>, <what the builders reported>" --cost "<measured cost and any gaps>"`

Filled in for a lint check:

`--catches "Last 8 changes, 11 gate runs, no failures after the builders finished. The builders left no counts." --cost "33 seconds in total. Tokens and wait were unmeasured."`

An approval a color asks for, such as `brief-approval`, is reduced the same way, with its name in `--expected-check`. In `--catches`, give the changes of that color asked to approve their brief and how many came back with a correction, from `brief_corrections`, and how many were approved within a minute of their question. In `--cost`, give the waits of those questions, from `asks`. What still guards that color without it goes in the proposal too: the reading of the brief, where the color has one, and the person's review of the change.

`--catches "Last 10 yellow changes asked to approve their brief, none came back corrected, none approved within a minute." --cost "212 minutes waiting for those yeses. Tokens unmeasured."`
