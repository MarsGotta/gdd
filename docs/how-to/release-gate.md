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

<!-- The chat excerpts and the screens of this page come from one real run of gdd 0.1.1 (main
at 30b50c4) on 2026-10-31, with Claude Code as the chat agent and Codex as the Reviewer: the
run of the tutorial. Before G4 the change parked three times on gdd 0.1.1 bugs and the agent
rebuilt it; the blocks trim the lines of that detour, and a Reviewer link with an absolute
path, with `...`. -->

```text
...
**What you'd approve:** the cart now gives three amounts: the subtotal before tax, the 21% tax
on the whole cart rounded to the cent (half a cent rounds up), and the total. Adding the same
piece again adds to its quantity. The code is merged on `main` and deployed to pre. Pre is
simulated here, so there's no video or screenshots.

**The evidence:**
- **Reviewer:** OK, with no new findings. ...
- **Full checks (G3):** all green, including security, dependencies and a scan for leaked
  secrets.
- **Demo:** I ran all 5 scenarios on `main` and they all pass:
  - a cart with several pieces: 60.50 + 12.71 = 73.21
  - an empty cart: 0.00
  - the two hidden examples (rounding down, and the same piece added twice)
  - the smoke test

**One gap:** ... Nothing can hit that today, because there's no screen where a customer types a
quantity yet. Left as is, though, a negative quantity would give a negative total.

**What I recommend:** say OK now, and make "a quantity must be at least 1" its own small change
before customers can choose quantities. The alternative is polish: one more Builder round to
add that check before going to production.

**Next step:** reply "OK", "polish", or "KO" (reject; production stays untouched).
```

It says what the change does with the numbers of your scenarios, what the gates and the
Reviewer said, and what it would leave for later, with a recommendation and its alternative.
The 5 scenarios are the 2 visible ones of `cart-total`, its 2 hidden examples and the smoke
scenario the Setup wrote (`features/smoke.feature`): G3 runs every scenario of the project, not
only the change's. The gap it names, a quantity of 0 or less, is the kind of remark polish is
for, or a small change of its own.

The inbox itself is the one message gdd keeps per change, in `.gdd/changes/CHANGE/inbox.md`.
At G4 it starts with how the change works. The full inbox of `cart-total`:

```text
# Decision Inbox · cart-total · Show the cart total, taxes included

Color yellow · lane full · state in-pre

## How it works
`src/shop/cart.py` holds catalogue lookup, accumulated quantities and totals.
It uses Decimal amounts and applies 21% tax once to the subtotal, rounding half up.
Acceptance steps follow the existing fixture pattern and call the package directly.
Adding the same piece again increases its quantity.

## Needs your decision
- Test mode: API assumed in shop.cart: TAX_RATE=Decimal('0.21'); Cart(catalogue: dict[str, Decimal]); Cart.add(piece, quantity) raises KeyError for a piece not in the catalogue; Cart.totals() -> Totals(subtotal, tax, total) as 2-decimal Decimal. Lint I001 on the new tests fails only while src/shop/cart.py is missing (Ruff then sees shop as third-party); the blank line before the shop import follows the package and is right once the module exists.
- G4 · Release Gate: check the pre demo and the change message, then answer your agent (or the sitting screen): OK promotes to production, KO rejects it (production untouched), polish with your notes runs one more Builder round. Your agent runs gdd resume cart-total --g4 ok, --g4 ko or --g4 polish --note <your notes>.

## Decided without you
- Money as Decimal, ROUND_HALF_UP to the cent on the cart subtotal's tax; tax rate a single constant (21%), confirmed by the owner
- No new red zone: prices and taxes are shown, no payment is taken; revisit when payments come in
...

## Parked
- Nothing.
```

- **How it works** is the Reviewer's short explanation of the code the change touched: where
  it sits, how it fits, the pattern it follows. It lets you approve what you understand
  without reading every line. When the Reviewer did not write one, the inbox says so.
- **Needs your decision** holds the G4 request and the notes the agents left for you on the
  way. Here the Builder in test mode described the code its tests expect, which the
  Orchestrator checked for you. The G4 line says what each answer does, and that your agent, or
  the sitting screen, runs the command for you: you answer in words.
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
│`src/shop/cart.py` holds catalogue lookup, accumulated quantities and totals.                     │
│It uses Decimal amounts and applies 21% tax once to the subtotal, rounding half up.               │
│Acceptance steps follow the existing fixture pattern and call the package directly.               │
│Adding the same piece again increases its quantity.                                               │
│Reviewer: OK                                                                                      │
...
│- No new blocking findings.                                                                       │
│Recorded G2 and G3 pass; mutation testing was skipped. No files modified or tests rerun during    │
│this read-only review.                                                                            │
│Demo: none, the project has no demo command                                                       │
│Decided without you                                                                               │
│- Money as Decimal, ROUND_HALF_UP to the cent on the cart subtotal's tax; tax rate a single       │
│constant (21%), confirmed by the owner                                                            │
│- No new red zone: prices and taxes are shown, no payment is taken; revisit when payments come in │
...
│Changelog: none set                                                                               │
...
│Agents: builder-test · sonnet 2m, reviewer · gpt-6-astra 1m, builder-build · sonnet 1m            │
│↑↓ scroll · o OK · k KO · p polish · q quit                                                       │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

From the top: how the change works, the Reviewer's verdict and its words, the demo (with its
numbers when the demo command wrote them, for example `Demo: 12 scenarios (12 passed)`), what
the agents decided without you, the changelog line, and where the change's time went: the flow
time, then each agent, each gate and each wait, further down. Here the Builder in test mode took
the longest.

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
The cart total is in production as **v0.1.0**, and the change is closed. Production is still
simulated, so nothing is deployed to a real server yet.

What shipped: the cart shows the subtotal, the 21% tax on the whole cart (rounded to the cent,
with half a cent rounding up), and the total. It's tagged and recorded on `main`.

**Cost:** about $0.91 for the Builders. The reviewer's and my own calls aren't priced, because
gdd has no price for those two models, so the real total is higher.
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
  changelog line gets the next version once the promotion passes: here `v0.1.0`, the first
  version of a project. gdd adds the line to `CHANGELOG.md` under the new version, commits it
  on `main` as `Release v0.1.0` and tags that commit `v0.1.0`. A later Feature raises the minor
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

<!-- The two refusals below were reproduced the same day with gdd 0.1.1, in the project of the
run once the change was closed, without agents. -->

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
