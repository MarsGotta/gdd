# Answer the Release Gate

Decide whether a change that waits at the Release Gate (G4) goes to production: look at what
it does, then answer OK, KO or polish.

## Before you start

- A change paused at G4. `bin/gdd status` shows it `in-pre`, the Board under Release as
  `waiting for your G4`, and `gdd launch` ended with `Paused at G4`.
  [Run a change](run-a-change.md) gets it there.

## When a change waits for you

G4 is the one stop after the build, and only some changes make it. A change waits for a
person at G4 when:

- it has a G1, because it creates or changes a scenario (`g4_if_g1 = true` for the full lane
  of its color in `gdd/rules.toml`, the default); or
- it is red: a red change always meets a person at G4, and gdd refuses a rules file that
  switches that off.

Any other change is promoted by the machines once pre is green. The `g4` field of
`bin/gdd classify CHANGE` says which case a change is in. A spike also stops at G4, where you
pick what its ADR says: see [Explore with a spike](spikes.md).

By the time a change waits, a lot has happened without you. It passed the Loop Gate (G2) and
the Merge Gate (G3), the Reviewer said OK, gdd merged it into `main` with its record in the
merge commit, built it once and deployed that build to pre. Then pre ran its tests and the
demo: video, screenshots, or for a library or a tool without a screen, the readable run of
its scenarios, left in `.gdd/changes/CHANGE/demo/`. What you approve is that build, as it runs
in pre.

`gdd init` leaves every release step empty, with `simulated = true`, in the `[release]` table
of `gdd/commands.toml`: until you fill `deploy_pre`, `demo` and the others, pre and production
are simulated and the sitting says `Demo: none, the project has no demo command`.

## 1. Read the Decision Inbox

The Decision Inbox is the one message gdd keeps per change. At G4 it starts with how the change
works:

```bash
bin/gdd inbox export-json
```

```text
# Decision Inbox · export-json · Export notes as JSON

Color yellow · lane full · state in-pre

## How it works
- `export_json` sits beside CSV export in `src/notes/export.py`.
- Python’s JSON serializer preserves note order and escapes string contents.
- Acceptance steps follow the existing fixture pattern.
- Unit tests cover empty lists, ordering, and embedded quotes.

## Needs your decision
- Lint I001 (import order) remains in tests/acceptance/steps/export_csv.py, left over from export-csv; not touched here. The 'the notes ... and ...' Given step is reused from export_csv.py.
...
- G4 · Release Gate: check the pre demo and the change message, then answer with gdd resume export-json --g4 ok (promote to production), --g4 ko (reject: production untouched) or --g4 polish --note <what to polish> (one more Builder round).

## Decided without you
- Reordered imports in tests/acceptance/steps/export_csv.py (ruff I001) so lint passes in G2; no behaviour change, same steps folder as the package paths.
...

## Parked
- Learn: export-csv closed with G2 lint green, but its unchanged tests/acceptance/steps/export_csv.py now fails ruff I001 at the same ruff version. Whether ruff treats 'notes' as first-party seems to depend on the environment. Consider pinning [tool.ruff] src = ["src"] or known-first-party = ["notes"] so lint gives the same result in every worktree. Not blocking export-json: the replan fixes the import order in a test round.
```

- **How it works** is the Reviewer's short explanation of the code the change touched: where
  it sits, how it fits, the pattern it follows. It lets you approve what you understand
  without reading every line. When the Reviewer did not write one, the inbox says so.
- **Needs your decision** holds the G4 request and the notes the agents left for you on the
  way. Here G2 lint failed on the import order of a step file from an earlier change, which the
  Builder in build mode may not edit, until the Orchestrator replanned and the Builder in test
  mode fixed it. The notes tell that story; read them to know what the change went through.
- **Decided without you** lists what the agents chose on their own, and the paths outside the
  plan that the scope check accepted.
- **Parked** lists what an agent set aside for later. Here it is a lesson about the lint
  settings, which does not stop the change.

When they apply, more sections follow: the security exceptions the change adds, the known debt
still failing or paid, and your earlier polish notes. The inbox is also saved in
`.gdd/changes/CHANGE/inbox.md`.

## 2. Open the G4 sitting

The sitting shows, on one screen, most of what you judge the change by:

```bash
bin/gdd sitting export-json
```

<!-- Real screen of gdd 0.1.0 at 100 columns: blank lines and part of the Reviewer's words and of
     the notes trimmed. -->
```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│export-json · Export notes as JSON · waiting for your G4                                          │
│How it works                                                                                      │
│- `export_json` sits beside CSV export in `src/notes/export.py`.                                  │
│- Python’s JSON serializer preserves note order and escapes string contents.                      │
│- Acceptance steps follow the existing fixture pattern.                                           │
│- Unit tests cover empty lists, ordering, and embedded quotes.                                    │
│Reviewer: OK                                                                                      │
│No blocking findings. The change meets the package scope, reuses the existing Given step, and     │
│makes real assertions. Recorded G2 and G3 results are green; tests were not rerun during this     │
...
│Demo: none, the project has no demo command                                                       │
│Decided without you                                                                               │
│- Reordered imports in tests/acceptance/steps/export_csv.py (ruff I001) so lint passes in G2; no  │
│behaviour change, same steps folder as the package paths.                                         │
...
│Changelog: none set                                                                               │
│Flow time 6m · active 6m (97%) · waiting 9s · between steps 1s                                    │
│Agents: orchestrator · opus 2m, builder-build · sonnet 2m, builder-test · sonnet 1m, reviewer ·   │
│gpt-6-astra 32s                                                                                   │
│Gates: security 1s, acceptance 1s, unit 1s, secrets 1s, architecture 0s, types 0s, acceptance_full│
│0s, lint 0s, dependencies 0s, lockfile 0s, path-guard 0s                                          │
│Waiting: at the sitting 5s, to start 4s                                                           │
│↑↓ scroll · o OK · k KO · p polish · q quit                                                       │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

From the top: how the change works, the Reviewer's verdict and its words, the demo (with its
numbers when the demo command wrote them, for example `Demo: 12 scenarios (12 passed)`), what
the agents decided without you, the changelog line set with
[`gdd changelog-line`](../reference/cli.md#gdd-changelog-line), and where the change's time
went: here most of it went to three Builder rounds on the lint failure and the Orchestrator's
replan. `Changelog: none set` means nobody set one: the merge commit then carries the default,
the title under the section of the change's type (`Added` for a Feature).

The agents' notes under *Needs your decision* and *Parked* do not appear on the sitting: read
them in the inbox first.

| Key | Does |
|---|---|
| `↑` `↓` | scrolls |
| `o` | OK: promote the change |
| `k` | KO: reject it, with your words |
| `p` | polish: one more Builder round, with your words |
| `q` | closes the sitting; nothing runs |

Every answer goes through a confirmation that shows the exact command and what follows.
Pressing `o` shows:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│This runs, as you:                                                                                │
│gdd resume export-json --g4 ok                                                                    │
│The change is promoted.                                                                           │
│y to run · n to go back                                                                           │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`y` closes the screen and runs that command in the terminal, so you see its progress; `n` goes
back and nothing runs. `k` and `p` first ask for your words, which they need:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│What should change?                                                                               │
│> _                                                                                               │
│Enter to continue · Backspace erases · Esc to go back                                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

KO then says "The change is rejected and stays out of production."; polish says "One more
Builder round with your words, then the gates and the demo again." If someone answered the same
change meanwhile, the sitting says so and runs nothing.

## 3. Check before you say OK

G4 is where *done* meets *good*. Every gate passed, so the question is no longer whether it
works but whether it is what you want. Look at:

1. **The demo in pre.** Does it do what the scenarios promised, on the real interface?
2. **How it works.** Does the explanation match what you asked for, and the shape you want the
   code to keep?
3. **The Reviewer's words.** An OK can still carry remarks worth a polish.
4. **Decided without you.** Each decision is one you could have made differently.
5. **Security exceptions and known debt**, when the inbox lists them.

If something is not right but close, prefer polish over KO: it keeps the work.

## 4. Answer

Answer from the main checkout, at the sitting or with one command. Only a person can: a
Builder or a Reviewer that runs `gdd resume` is refused.

### OK: promote it

```bash
bin/gdd resume export-json --g4 ok
```

```text
Released v0.2.0
Change export-json closed. Message: .gdd/changes/export-json/inbox.md · Tokens: gdd report --change export-json
```

gdd records your OK, promotes **the same build** that ran in pre to production, without
rebuilding it, then observes production. When the observation passes, the change closes and
its worktree goes; its branch stays.

- **Versions.** With `[versions] mode = "auto"` (the default), a promoted change with a
  changelog line gets the next version tag once the promotion passes: here `v0.2.0`, because
  the project was at `v0.1.0` and a Feature goes under `Added`, which raises the minor version. See [Release versions and keep a changelog](releases-and-changelog.md).
- **Rollback.** When the observation fails, gdd runs the `rollback` step back to the previous
  version, the change ends `rolled-back`, and its inbox asks for a hotfix: "Production failed
  while observing: rolled back to the previous version. A hotfix is needed."
- **A failed promotion** leaves production untouched and the change still paused at G4, with
  the reason in its inbox. Fix the cause and answer OK again: the same OK retries it.
- **A promotion cut short** (the machine stopped mid-way) parks the change instead of guessing,
  with a message to look at production first, so nothing is deployed twice.

### KO: reject it

```bash
bin/gdd resume export-json --g4 ko --note "Not what I asked"
```

gdd records your KO and your words, leaves production untouched and closes the worktree; the
change ends `rejected`. KO does not touch `main`: the merge commit from G3 stays there.

### Polish: one more round

```bash
bin/gdd resume export-json --g4 polish --note "Indent the JSON, one note per line"
```

Polish needs your notes; without `--note` gdd refuses:

```text
error: polish needs your notes: gdd resume export-json --g4 polish --note ...
```

The Builder gets one more round in build mode with your notes, then the change goes through G2,
G3, the Reviewer, the merge when there is something new, and pre again, and pauses at G4 with
`Paused at G4`. Answer it again as before. Polish replaces a KO rather than adding a stop, and
your notes stay in the inbox under Polish. [`gdd learn`](../reference/cli.md#gdd-learn) lists
the notes that repeat across changes, so they become a line of `docs/standards.md` or a check.

The `--note` words are the rest of the line, so quotes are optional. A change that is not
waiting says so:

```text
error: Change export-json is not paused at G4.
```

For a spike with variants, `--variant N` says which one OK brings to `main` or polish builds
again.

## Next steps

- [Get a parked change going again](parked-change.md): when a change stops before G4.
- [Release versions and keep a changelog](releases-and-changelog.md): tags, `gdd release` and
  `CHANGELOG.md`.
- [Few stops for the human](../explanation/human-stops.md): why G4 is the only stop after the
  build, and why done is not good.
