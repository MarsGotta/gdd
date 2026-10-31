# Review a change at G4

Use this page when a change waits for you at the Release Gate (G4): the Orchestrator brings it
back to the chat, or the Board shows it as `waiting for your G4`. By then the machines have
checked that the change works. G4 is where you check that it is what you wanted, before it
reaches production. You look at what it does and answer OK, KO or polish, in words.

## When a change waits for you

G4 is the one stop after the build, and only some changes make it. A change waits for a person
at G4 when:

- it has a G1, because it creates or changes a scenario (`g4_if_g1 = true` for the full lane
  of its color in `gdd/rules.toml`, the default); or
- it is red: a red change always meets a person at G4, and gdd refuses a rules file that
  switches that off.

Any other change is promoted by the machines once pre is green. A spike also stops at G4,
where you pick what its ADR says: see [Explore with a spike](spikes.md).

By the time a change waits, a lot has happened without you. It passed the Loop Gate (G2) and
the Merge Gate (G3), and the Reviewer said OK. Then gdd merged it into `main`, with its record
in the merge commit, built it once and deployed that build to pre, the copy of the project
that runs before production. Pre ran its tests and the demo: video, screenshots, or for a
library or a tool without a screen, the readable run of its scenarios, left in
`.gdd/changes/CHANGE/demo/`. What you approve is that build, as it runs in pre.

`gdd init` leaves every release step empty, with `simulated = true`, in the `[release]` table
of `gdd/commands.toml`: until you fill `deploy_pre`, `demo` and the others, pre and production
are simulated and there is no demo to look at.

## 1. Read the Decision Inbox

When the change pauses at G4, the Orchestrator brings its Decision Inbox and its demo into the
chat, in plain words, and asks for your answer. For `cart-total`, the change that shows the
cart total of `shop`, the agent wrote:

<!-- The chat excerpts and the screens of this page come from one real run of gdd 0.1.0 (main
at e244577) on 2026-10-31, with Claude Code as the chat agent and Codex as the Reviewer: the
run of the tutorial. Paths are shown under /home/me. -->

```text
The **cart-total** change is built and checked, and it's waiting for your **G4** answer.

**What it does:** a cart adds up the pieces in it and shows two figures, the total and how much
of that total is tax.
- 1 Blue mug (24.20) + 2 Wide bowls (36.30 each) gives a **total of 96.80, of which 16.80 is
  tax**.
...
**Checks:**
- **All 6 scenarios pass**, including the 2 hidden examples the Builder never saw. That shows
  it implemented the rule rather than copying the numbers.
- **The other merge checks pass:** lint, types, unit tests, security, secrets and dependencies.
- **Mutation testing was skipped,** as the rules say for yellow changes. ...
- **The Reviewer from another model family said OK** with no findings.
- **It's merged into `main`.** The test server is simulated, so there's no page or video to
  look at yet. ...

**Two things you should know:**
- The inbox asks you to approve the cart's technical interface: how other code adds pieces and
  reads the totals. That's a code detail; I've checked it and it's fine, so you don't need to
  decide anything there.
- The cart doesn't yet refuse odd input like a quantity of 0 or a negative price. Nothing
  allows that today, but I'd add the rule when pieces are added from a real page.

**Next step:** reply **OK** to send it to production (simulated for now), **KO** to reject it,
or **polish** with what you'd like changed.
```

It says what the change does with the numbers of your scenarios, what the gates and the
Reviewer said, and what it read in the inbox for you. The 6 scenarios are the 3 visible ones of
`cart-total`, its 2 hidden examples and the smoke scenario the Setup wrote
(`features/smoke.feature`): G3 runs every scenario of the project, not only the change's.

The inbox itself is the one message gdd keeps per change, in `.gdd/changes/CHANGE/inbox.md`.
At G4 it starts with how the change works. The full inbox of `cart-total`:

```text
# Decision Inbox · cart-total · Show the cart total, taxes included

Color yellow · lane full · state in-pre

## How it works
- `src/shop/cart.py` adds a plain Python cart, independent of the web layer.
- Totals sum tax-inclusive prices multiplied by quantities using `Decimal`.
- Included tax uses the supplied rate, calculated once on the total and rounded half up.
- Acceptance steps follow the existing fixture pattern; unit tests cover rounding, alternative rates and empty carts.

## Needs your decision
- API expected by the tests: shop.cart.Cart(tax_rate: Decimal) with add(name: str, price: Decimal, quantity: int), and Decimal properties total and tax_included, both quantized to two places (str() shows e.g. 96.80). The rate is a percentage (Decimal(21)).
- G4 · Release Gate: check the pre demo and the change message, then answer with gdd resume cart-total --g4 ok (promote to production), --g4 ko (reject: production untouched) or --g4 polish --note <what to polish> (one more Builder round).

## Decided without you
- No human plan: the change is yellow. Money as Decimal, tax rate given to the cart (not hard-coded), tax computed once on the cart total and rounded half up to the cent.
- Euros: scenarios keep amounts without the euro sign, as approved; the currency shows up when the cart reaches a page.

## Parked
- Nothing.
```

- **How it works** is the Reviewer's short explanation of the code the change touched: where
  it sits, how it fits, the pattern it follows. It lets you approve what you understand
  without reading every line. When the Reviewer did not write one, the inbox says so.
- **Needs your decision** holds the G4 request and the notes the agents left for you on the
  way. Here the Builder in test mode described the code its tests expect, which the
  Orchestrator checked and summed up for you. The G4 line names the command your answer
  becomes: the Orchestrator runs it, you answer in words.
- **Decided without you** lists what the agents chose on their own, and the paths outside the
  plan that the scope check accepted.
- **Parked** lists what an agent set aside for later: nothing here.

When they apply, more sections follow: the security exceptions the change adds, the known debt
still failing or paid, and your earlier polish notes.

## 2. Or open the G4 sitting

If you prefer one screen with most of what you judge the change by, open the sitting in a
terminal:

```bash
gdd sitting cart-total
```

You should see (blank lines trimmed):

```text
┌ gdd · Sitting · shop ────────────────────────────────────────────────────────────────────────────┐
│cart-total · Show the cart total, taxes included · waiting for your G4                            │
│How it works                                                                                      │
│- `src/shop/cart.py` adds a plain Python cart, independent of the web layer.                      │
│- Totals sum tax-inclusive prices multiplied by quantities using `Decimal`.                       │
│- Included tax uses the supplied rate, calculated once on the total and rounded half up.          │
│- Acceptance steps follow the existing fixture pattern; unit tests cover rounding, alternative    │
│rates and empty carts.                                                                            │
│Reviewer: OK                                                                                      │
│No blocking findings. The implementation matches the package, and scenario steps assert exact     │
│amounts with two decimal places. No duplicated steps or example-specific shortcuts found.         │
│Recorded G2 and G3 pass; mutation testing was skipped. This review was read-only; no tests were   │
│rerun.                                                                                            │
│Demo: none, the project has no demo command                                                       │
│Decided without you                                                                               │
│- No human plan: the change is yellow. Money as Decimal, tax rate given to the cart (not          │
│hard-coded), tax computed once on the cart total and rounded half up to the cent.                 │
│- Euros: scenarios keep amounts without the euro sign, as approved; the currency shows up when the│
│cart reaches a page.                                                                              │
│Changelog: none set                                                                               │
│Flow time 3m · active 2m (59%) · waiting 1m · between steps 0s                                    │
│Agents: builder-test · sonnet 51s, reviewer · gpt-6-astra 30s, builder-build · sonnet 17s         │
│Gates: security 1s, secrets 0s, acceptance 0s, acceptance_full 0s, unit 0s, architecture 0s, lint │
│0s, types 0s, dependencies 0s, lockfile 0s, path-guard 0s                                         │
│Waiting: at the sitting 55s, to start 15s                                                         │
│↑↓ scroll · o OK · k KO · p polish · q quit                                                       │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

From the top: how the change works, the Reviewer's verdict and its words, the demo (with its
numbers when the demo command wrote them, for example `Demo: 12 scenarios (12 passed)`), what
the agents decided without you, the changelog line, and where the change's time went. Here the
Builder in test mode took the longest, and the change waited 55 seconds for your OK at the
first sitting.

Two lines say "nothing yet". `Demo: none, the project has no demo command` because `shop` has
not filled the `demo` step of `[release]` yet, so pre left no demo files. `Changelog: none set`
means nobody set one: the merge commit then carries the default, the title under the section
of the change's type (`Added` for a Feature).

The agents' notes under *Needs your decision* and *Parked* do not appear on the sitting: read
them in the inbox first.

## 3. Check before you say OK

Every gate passed, so the question at G4 is no longer whether the change works but whether it
is what you want. Before you answer, look at:

1. **The demo in pre.** Does it do what the scenarios promised, on the real interface?
2. **How it works.** Does the explanation match what you asked for, and the shape you want the
   code to keep?
3. **The Reviewer's words.** An OK can still carry remarks worth a polish.
4. **Decided without you.** Each decision is one you could have made differently.
5. **Security exceptions and known debt**, when the inbox lists them.

If something is close but not right, answer polish rather than KO: polish keeps the work.

## 4. Answer

Answer the Orchestrator in the chat, in your own words. It records your answer for you.

| You say | What happens |
|---|---|
| OK | The change goes to production. |
| KO, and why | The change is rejected and stays out of production. |
| Polish, and what to polish | One more Builder round with your words, then the gates and the demo again, and the change waits at G4 once more. |

For `cart-total`, the answer was one word:

```text
OK
```

The agent answered:

```text
**cart-total** is released as **v0.1.0**, and the change is closed. Production is still
simulated, so nothing is live on the web yet. The shop's core now has a cart that shows the
total and the tax included in it.

**What it cost:** the report shows **$0.39** in total. That figure covers only the Builder's
two rounds (writing the tests, then the code). The Reviewer and my own work show $0.00, which
looks like they weren't priced in the report rather than free. Your answers stopped the process
twice for a human decision: the scenarios and G4.
...
```

A polish needs your words, for example `Polish: show the total with the euro sign.` A KO
needs the reason.

At the sitting, `o` is OK, `k` KO and `p` polish. `k` and `p` first ask `What should change?`
and wait for your words. Every answer then goes through a confirmation that shows the exact
command and what follows. Pressing `o` shows:

```text
┌ gdd · Sitting · shop ────────────────────────────────────────────────────────────────────────────┐
│This runs, as you:                                                                                │
│gdd resume cart-total --g4 ok                                                                     │
│The change is promoted.                                                                           │
│y to run · n to go back                                                                           │
│↑↓ scroll · q quit                                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`y` closes the screen and runs that command in the terminal, so you see its progress; `n` goes
back and nothing runs. KO says "The change is rejected and stays out of production."; polish
says "One more Builder round with your words, then the gates and the demo again." Only a person
can answer G4: a Builder or a Reviewer that tries is refused.

### OK: promote it

gdd records your OK, promotes **the same build** that ran in pre to production, without
rebuilding it, then observes production. When the observation passes, the change closes and
its worktree goes; its branch stays. For `cart-total`, gdd printed:

```text
Released v0.1.0
Change cart-total closed. Message: .gdd/changes/cart-total/inbox.md · Tokens: gdd report --change cart-total
```

- **Versions.** With `[versions] mode = "auto"` (the default), a promoted change with a
  changelog line gets the next version tag once the promotion passes: here `v0.1.0`, the first
  version of a project, on the merge commit of `cart-total`. A later Feature raises the minor
  version. See [Release versions and keep a changelog](releases-and-changelog.md).
- **Rollback.** When the observation fails, gdd runs the `rollback` step back to the previous
  version, the change ends `rolled-back`, and its inbox asks for a hotfix: "Production failed
  while observing: rolled back to the previous version. A hotfix is needed." Run `/gdd-hotfix`
  (`$gdd-hotfix` in Codex; see [the hotfix skill](../reference/skills/hotfix.md)).

### KO: reject it

gdd records your KO and your words, leaves production untouched and removes the worktree. The
change ends `rejected`.

KO does not undo the merge. gdd merged the change into `main` right after the Reviewer's OK,
before pre and G4, so after a KO `main` and pre still have it and production does not. A later
change merges on top of it, so its release would carry this code to production too. If you
want the code out of `main`, ask the Orchestrator for a change that removes it.

### Polish: one more round

The Builder gets one more round in build mode with your words, then the change goes through
G2, G3, the Reviewer, the merge when there is something new, and pre again, and pauses at G4.
Answer it again as before. Polish replaces a KO rather than adding a stop, and your words stay
in the inbox under Polish. Learn lists the polish notes that repeat across changes, so they
become a line of `docs/standards.md` or a check (see [Measure the work](measure.md)).

## When it says no

- **A promotion fails.** Production stays untouched and the change stays paused at G4, with
  the reason in its inbox. Fix the cause and answer OK again: the same OK retries it.
- **A promotion is cut short** (the machine stopped mid-way). gdd parks the change instead of
  guessing, with a message to look at production first, so nothing is deployed twice.

<!-- The two refusals below were reproduced the same day in a throwaway copy of the same
project, without agents and without a launch. -->

- **Polish without words.** gdd refuses:

  ```text
  error: polish needs your notes: gdd resume cart-total --g4 polish --note ...
  ```

- **The change is not waiting any more**, for example because someone answered it meanwhile.
  gdd says so and runs nothing. The sitting says, for a change closed meanwhile,
  `CHANGE is closed now: nothing was run`; an answer in the chat gets:

  ```text
  error: Change cart-total is not paused at G4.
  ```

## Under the hood

Your answer becomes [`gdd resume CHANGE --g4 ok`](../reference/cli/resume.md),
`--g4 ko --note ...` or `--g4 polish --note ...`; the words after `--note` are the rest of the
line. For a spike with variants, `--variant N` says which one OK brings to `main` or polish
builds again. [`gdd inbox`](../reference/cli/inbox.md) prints the Decision Inbox, also saved in
`.gdd/changes/CHANGE/inbox.md`, and [`gdd sitting`](../reference/cli/sitting.md) opens the G4
screen. The changelog line comes from
[`gdd changelog-line`](../reference/cli/changelog-line.md).

## Next steps

- [Get a parked change going again](parked-change.md): when a change stops before G4.
- [Release versions and keep a changelog](releases-and-changelog.md): tags, `gdd release` and
  `CHANGELOG.md`.
- [Few stops for the human](../explanation/human-stops.md): why G4 is the only stop after the
  build, and why done is not good.
