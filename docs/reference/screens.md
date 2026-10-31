# Screens

gdd has three screens in the terminal: the Setup form of `gdd init`, the Board and the
sitting, which has a second page for the Release Gate (G4). This page shows each one, its keys,
and what each key writes. Consult it when you answer on a screen instead of in the chat with
your agent.

<!-- Every capture on this page is a real screen of gdd 0.1.0 in a terminal of 100 x 30, from a
     throwaway project, with the blank lines trimmed; the G4 sitting is the one of the
     tutorial's real run, from how-to/release-gate.md. The form's summary and output, the empty
     Board and the sitting with nothing waiting were captured again after gdd 0.1.0 (commit
     07b1cd6), in a project called shop. -->

## Before you start

Each screen needs a terminal and a person. Run from a script or a pipe, the command refuses and
says what to run instead; the messages are under each screen. An agent that gdd started never
opens one (see [Roles](roles.md#what-an-agent-may-run)). `Ctrl-C` closes any of them at any
point.

## The Setup form

The Setup form sets up a project from your answers instead of flags.
[`gdd init`](cli/init.md) opens it in a terminal when you give it no `--platform`. It asks seven
questions, one per step, shows a summary, and writes nothing until you press `Enter`
on the summary. The flags `--agents`, `--language`, `--description` and `--sensitive`, when
you give them, come filled in; the name, the platform and the stacks are asked again.

```bash
gdd init
```

```text
┌ gdd init · Step 1 of 7 · Project name ───────────────────────────────────────────────────────────┐
│What is the project called?                                                                       │
│> █                                                                                               │
│Enter: next · Ctrl-C: quit                                                                        │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Step | Asks | You answer with |
|---|---|---|
| 1 · Project name | What is the project called? | text; `Enter` stays here until you type a name |
| 2 · Description | What is it, in a few words? It may stay empty. | text |
| 3 · Platform | Where does it run? | `↑` `↓` to `mobile`, `desktop`, `web`, `cli` or `other`, then `Enter` |
| 4 · Stacks | Which technology does it use? | `Space` checks `rust`, `typescript`, `python` or `Not sure` |
| 5 · Coding agents | Which coding agents? The first checked builds. | `Space` checks `claude`, `codex`, `opencode` |
| 6 · Language | Which language do the agents use with you? | text, `en` by default |
| 7 · Sensitive areas | What would be serious if it failed? One area per line. | text, `Enter` after each area |

The steps with a list show a box per item; `>` marks the cursor:

```text
┌ gdd init · Step 4 of 7 · Stacks ─────────────────────────────────────────────────────────────────┐
│Which technology does it use? Pick one or more, or Not sure.                                      │
│  [ ] rust                                                                                        │
│  [ ] typescript                                                                                  │
│> [x] python                                                                                      │
│  [ ] Not sure                                                                                    │
│Up/Down: move · Space: check · Enter: next · Esc: back · Ctrl-C: quit                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The coding agents found on your `PATH` come checked. Uncheck the ones you do not want; at
least one must stay. The sensitive areas become lines of the Project Brief, which the discover
skill later turns into red zones:

```text
┌ gdd init · Step 7 of 7 · Sensitive areas ────────────────────────────────────────────────────────┐
│What would be serious if it failed? One area per line.                                            │
│  - Losing notes                                                                                  │
│> A note leaking to another user█                                                                 │
│Enter: add, or go on when empty · Backspace: edit the last · Esc: back · Ctrl-C: quit             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Keys

| Key | Does |
|---|---|
| any letter, `Space`, `Backspace` | types or erases, on the steps that take text |
| `↑` `↓` | moves the cursor, on the steps with a list |
| `Space` | checks or unchecks the item at the cursor (stacks, coding agents) |
| `Enter` | goes to the next step when this one is answered; else says what it still needs, in red, and stays. On step 7 it adds the area typed, and goes on from an empty line |
| `Backspace` on an empty line of step 7 | takes back the last area, to edit it |
| `Esc` | goes back one step, keeping every answer |
| `Ctrl-C` | quits: nothing is written |

What `Enter` says when a step is not answered: `Type a name: the project needs one`, `Pick at
least one stack, or Not sure` and `Pick at least one coding agent`.

### The summary

The summary shows every answer before anything is written:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Name: shop                                                                                        │
│Description: A small web shop: a catalog, a cart and a checkout.                                  │
│Platform: web                                                                                     │
│Stacks: python                                                                                    │
│Agents: claude, codex                                                                             │
│Language: en                                                                                      │
│Sensitive: Payments and customer data.                                                            │
│Enter to set up · Esc to go back                                                                  │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` sets the project up, as `gdd init` with the same answers as flags would: it writes the
harness, the Brief, the skeleton of each stack and the skills of each coding agent, and commits
them in one commit, `gdd init 0.1.0`. Then it runs the first install of each stack, as
[`gdd bootstrap`](cli/bootstrap.md) does, and commits the lock files in a second commit,
`gdd bootstrap`. It lists every file it wrote, then what the install printed, and ends by
sending you to your coding agent:

```text
gdd/project.toml
gdd/commands.toml
gdd/rules.toml
gdd/zones.toml
gdd/models.toml
gdd/generated.toml
docs/brief.md
.github/workflows/gdd.yml
AGENTS.md
CLAUDE.md
...
.claude/skills/gdd-setup/SKILL.md
.claude/skills/gdd-status/SKILL.md
.claude/skills/gdd-stack-python/SKILL.md
...
.agents/skills/gdd-stack-python/SKILL.md
python: uv lock && uv sync --locked
Using CPython 3.13.15
Resolved 40 packages in 15ms
...
Committed: uv.lock
Next: start your coding agent here and run the gdd-setup command (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

When an install fails, the harness stays committed: the form says which stack failed and that
`gdd bootstrap` runs the install again once you fixed the cause.

When every coding agent you checked is of one model family, the Reviewer would be of the same
family as the Builder, which GDD avoids. The summary asks for `y` instead of `Enter`, and only
`y` sets the project up:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Name: shop                                                                                        │
│Description: none                                                                                 │
│Platform: web                                                                                     │
│Stacks: rust                                                                                      │
│Agents: claude                                                                                    │
│Language: en                                                                                      │
│Sensitive: none                                                                                   │
│The Reviewer would be of the same model family as the Builder: press y to accept it               │
│Esc to go back                                                                                    │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

When you checked `Not sure` among the stacks, `Enter` writes nothing. gdd prints the
`gdd init` command with all your other answers as flags, to run once you know the technology:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Not sure of the technology yet: once you know it, run                                             │
│gdd init --name notes --platform cli --agents claude codex --language en --stack <pick one>       │
│Your coding agent can help you choose it.                                                         │
│Enter to finish · Esc to go back                                                                  │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Ctrl-C` on any step ends with this line and exit code 1:

```text
gdd init stopped: nothing was written
```

Without a terminal:

```text
error: gdd init opens its Setup form only in a terminal; else give it the flags --name, --platform and --stack
```

## The Board

[`gdd board`](cli/board.md) shows every change of the project by phase. It only reads: no key
writes anything. `gdd` with no command opens it too, when a person runs it in a terminal;
anywhere else, and for an agent, `gdd` alone prints the help.

```bash
gdd board
```

```text
┌ gdd · Board · notes ─────────────────────────────────────────────────────────────────────────────┐
│Discover (2)                                                                                      │
│› export-csv · yellow · Export notes as CSV · open · no gates yet                                 │
│  delete-notes · red · Delete a note · open · no gates yet                                        │
│Build (0)                                                                                         │
│Release (0)                                                                                       │
│Done (0) · d to show                                                                              │
│↑↓ move · Enter details · d done · t time · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Each line is one change: its id, color, title, state and the gates it passed. The phases:

| Phase | The changes that are |
|---|---|
| Discover | `open`: created, not launched yet |
| Build | `prepared`, `building`, `polishing` or `parked` |
| Release | `in-pre`, shown as `waiting for your G4` |
| Done | `closed`, `rejected` or `rolled-back`; folded until you press `d` |

A project with no change yet says so, and says how to start one: ask your agent.

```text
┌ gdd · Board · shop ──────────────────────────────────────────────────────────────────────────────┐
│No changes yet · ask your agent: /gdd-change <what you want> ($gdd-change in Codex)               │
│↑↓ move · Enter details · d done · t time · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` opens the details of the selected change: its type, color, state, gates, planned
paths, the path of its Decision Inbox and where its time went.

```text
┌ gdd · Board · notes ─────────────────────────────────────────────────────────────────────────────┐
│delete-notes · Delete a note                                                                      │
│Type: Feature                                                                                     │
│Color: red                                                                                        │
│State: open                                                                                       │
│Gates: no gates yet                                                                               │
│Paths: src/notes/**, tests/**                                                                     │
│Inbox: .gdd/changes/delete-notes/inbox.md                                                         │
│Time: nothing measured yet                                                                        │
│Esc to go back · q quit                                                                           │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`t` opens the time of the project: the flow time of each change, how much of it was active
(agents, gates, release), waiting and between steps.
[Measure tokens, time and delivery](../how-to/measure.md#2-see-where-the-time-goes) explains
each number.

```text
┌ gdd · Board · notes ─────────────────────────────────────────────────────────────────────────────┐
│Where the time goes                                                                               │
│Project: 2 change(s) · flow time 0s · active 0s (0%) · waiting 0s · between steps 0s              │
│export-csv · Export notes as CSV · in progress                                                    │
│  Flow time 0s · active 0s (0%) · waiting 0s · between steps 0s                                   │
│delete-notes · Delete a note · in progress                                                        │
│  Flow time 0s · active 0s (0%) · waiting 0s · between steps 0s                                   │
│↑↓ scroll · Esc to go back · q quit                                                               │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Keys

| Key | On the list | In the details or the time |
|---|---|---|
| `↑` `↓` | moves the selection | scrolls |
| `Enter` | opens the details of the selected change | |
| `t` | opens the time of the project | |
| `d` | shows or folds Done | |
| `Esc` | | back to the list |
| `q`, `Ctrl-C` | closes the Board | closes the Board |

Without a terminal:

```text
error: gdd board needs a terminal; gdd status lists the same changes
```

## The sitting

[`gdd sitting CHANGE`](cli/sitting.md) opens everything a change needs from you before the
build, on one page: the scenarios as the change's branch has them, the red zones when they
wait for approval, and the plan of a red change. The change skill shows you the same things in
the chat; the screen records the same decisions.

```bash
gdd sitting export-csv
```

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│export-csv · Export notes as CSV · yellow                                                         │
│Scenarios                                                                                         │
│features/export_csv.feature                                                                       │
│Feature: Export notes as CSV                                                                      │
│  So that I can open my notes in a spreadsheet, notes writes them as CSV: a header line,          │
│  then one line per note, in the order they were written.                                         │
│  Scenario: Each note becomes one line                                                            │
│    Given the notes "Buy milk" and "Call the bank"                                                │
│    When I export the notes as CSV                                                                │
│    Then the CSV is:                                                                              │
│      """                                                                                         │
│      note                                                                                        │
│      Buy milk                                                                                    │
│      Call the bank                                                                               │
│      """                                                                                         │
│features/hidden/export_csv.feature · hidden: the Builders never see it                            │
│Feature: Export notes as CSV (hidden examples)                                                    │
│  Scenario: Other notes, same rule                                                                │
│    Given the notes "Water the plants" and "Pay the rent"                                         │
│    When I export the notes as CSV                                                                │
│    Then the CSV is:                                                                              │
│      """                                                                                         │
│      note                                                                                        │
│↑↓ scroll · a approve · c ask for changes · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The page scrolls with `↑` `↓`. Below the scenarios come the hidden examples, marked `hidden:
the Builders never see it`, then the red zones and the plan:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
...
│features/hidden/export_csv.feature · hidden: the Builders never see it                            │
│Feature: Export notes as CSV (hidden examples)                                                    │
│  Scenario: Other notes, same rule                                                                │
│    Given the notes "Water the plants" and "Pay the rent"                                         │
│    When I export the notes as CSV                                                                │
│    Then the CSV is:                                                                              │
│      """                                                                                         │
│      note                                                                                        │
│      Water the plants                                                                            │
│      Pay the rent                                                                                │
│      """                                                                                         │
│Red zones to approve                                                                              │
│gdd/**, .github/**, .claude/**, .agents/**, .opencode/**, .gdd/bin/** · The harness itself:       │
│touching the gates is touching the safety of the method                                           │
│pyproject.toml, uv.lock, osv-scanner.toml, osv-scanner-licenses.toml · Dependencies and toolchain:│
│supply chain                                                                                      │
│Plan: not needed for a yellow change                                                              │
│↑↓ scroll · a approve · c ask for changes · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

A red change shows its plan, from `.gdd/changes/CHANGE/plan.md`, at the end. A change with no
scenarios says `No scenario files on change/CHANGE`, and red zones already approved say
`Red zones: approved`:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│delete-notes · Delete a note · red                                                                │
│Scenarios                                                                                         │
│No scenario files on change/delete-notes                                                          │
│Red zones: approved                                                                               │
│Plan                                                                                              │
│# Plan · delete-notes                                                                             │
│What: `notes delete N` removes note N and keeps the others in order.                              │
│How: a function `delete(notes, n)` in src/notes/store.py, called by the command line.             │
│Risks: deleting the wrong note loses data. The command asks before it deletes.                    │
│Roll back: revert the merge commit; the notes file format does not change.                        │
│↑↓ scroll · a approve · c ask for changes · q quit                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

When the scenarios are already approved and nothing else waits for you, the sitting says so and
offers no approval: only `↑` `↓` and `q` work.

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

### Approve

`a` shows exactly what it will record, as you. Nothing is written yet:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│This records, as you:                                                                             │
│- G1 ok on export-csv, sealing features/export_csv.feature, features/hidden/export_csv.feature    │
│- red zones ok: 2 red zone(s)                                                                     │
│y to record · n to go back                                                                        │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`y` records it, and the page says what was recorded and what comes next:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│Recorded                                                                                          │
│G1 ok on export-csv (human).                                                                      │
│Red zones ok (2 red zone(s)).                                                                     │
│Next: tell your agent it is approved (it runs gdd launch export-csv)                              │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

| What the sitting showed | What `y` records | Where |
|---|---|---|
| Scenarios | G1 OK, sealing each scenario file: gdd keeps its hash, so no agent can change it without a new approval | the change's `change.json` (`gates.G1`) and `.gdd/log.jsonl` |
| Red zones to approve | the approval of the red zones | `.gdd/zones.json` and the log |
| A plan | the approval of the plan | `change.json` (`gates.plan`) and the log |

These are the same records as [`gdd gate`](cli/gate.md) with `--by human --note sitting`. G1
is recorded from a throwaway worktree of the change's branch, so your checkout stays on
`main`. For the red change above, `a` and `y` record only its plan:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│Recorded                                                                                          │
│plan ok on delete-notes (human).                                                                  │
│Next: tell your agent it is approved (it runs gdd launch delete-notes)                            │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

When the scenarios, the red zones or the plan changed after you opened the page, `y` records
nothing: the page shows what the project has now, under `This changed after you read it: read
it again before approving`.

### Ask for changes

`c` asks for your words:

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│What should change?                                                                               │
│> _                                                                                               │
│Enter to continue · Backspace erases · Esc to go back                                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`Enter` shows what it will record, and `y` records it: G1 KO on the change, with your words as
its note, in the change's record and the log.

```text
┌ gdd · Sitting · notes ───────────────────────────────────────────────────────────────────────────┐
│G1 ko on export-csv: Add an example with an empty list                                            │
│y to record · n to go back                                                                        │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Keys

| Key | Does |
|---|---|
| `↑` `↓` | scrolls the page |
| `a` | approve: shows what it will record |
| `c` | ask for changes: asks for your words |
| `y` | on a confirmation, records it |
| `n`, `Esc` | on a confirmation or the text line, goes back; nothing is recorded |
| `Enter` | on the text line, goes on to the confirmation, once you typed something |
| `Backspace` | on the text line, erases |
| `q`, `Ctrl-C` | closes the sitting |

Once a change is launched, its sitting is over: the page says `CHANGE is STATE: its sitting is
over`, and only scrolls. A change waiting for you at G4 opens the G4 page instead.

Without a terminal, the command says how to record the same decisions:

```text
error: gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok export-csv --by human (and gdd gate zones ok --by human, gdd gate plan ok export-csv --by human for a red change)
```

## The G4 sitting

When a change waits at the Release Gate, `in-pre`, `gdd sitting CHANGE` shows what you judge
it by, in this order:

- how it works, in the Reviewer's words;
- the Reviewer's verdict;
- the demo;
- what the agents decided without you;
- the changelog line;
- where the change's time went.

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

`Demo: none, the project has no demo command` means `demo` in `[release]` of
`gdd/commands.toml` is empty; `How it works: the Reviewer did not explain the change` and
`Reviewer: no review yet` stand in when the review lacks them. The agents' notes under *Needs
your decision* and *Parked* are not on this page: read them in the Decision Inbox,
[`gdd inbox CHANGE`](cli/inbox.md).

### Answer

Every answer goes through a confirmation that shows the exact command it runs, as you, and
what follows. Nothing runs before `y`.

| Key | Answer | Asks first | Then the confirmation says |
|---|---|---|---|
| `o` | OK | nothing | `gdd resume CHANGE --g4 ok` · The change is promoted. |
| `k` | KO | `What should change?` | `gdd resume CHANGE --g4 ko --note "WORDS"` · The change is rejected and stays out of production. |
| `p` | polish | `What should change?` | `gdd resume CHANGE --g4 polish --note "WORDS"` · One more Builder round with your words, then the gates and the demo again. The agents run now: it takes minutes. |

`y` on the confirmation closes the screen and runs that [`gdd resume`](cli/resume.md) in the
terminal, so you see its progress. `n` or `Esc` goes back and runs nothing. When someone
answered the change meanwhile, `y` says `CHANGE is STATE now: nothing was run`.
[Answer the Release Gate](../how-to/release-gate.md) shows the
confirmation and the text line.

A spike has no Reviewer, demo or changelog line: its page says so, and OK brings only its ADR
files under `docs/adr/` to `main`. On a spike with variants, `o` and `p` first ask `Which
variant? Its number:`. See [Explore with a spike](../how-to/spikes.md).

### Keys

| Key | Does |
|---|---|
| `↑` `↓` | scrolls |
| `o` | OK: promote the change |
| `k` | KO: reject it, with your words |
| `p` | polish: one more Builder round, with your words |
| `y` | on the confirmation, closes the screen and runs the answer |
| `n`, `Esc` | on the confirmation or the text line, goes back; nothing runs |
| `q`, `Ctrl-C` | closes the sitting; nothing runs |

## See also

- [Approve the scenarios](../how-to/write-scenarios.md): what to look for at the sitting.
- [Answer the Release Gate](../how-to/release-gate.md): what to check before OK.
- [The change skill](skills/change.md): the same sitting and G4, in the chat.
- [Files](files.md): `change.json`, `zones.json` and the log the sitting writes.
