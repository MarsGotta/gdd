# Few stops for the human

Why does GDD ask you so little, and why at these moments and not others? Because your time is
the scarcest resource in the system, and most of what agents produce can be checked by
machines. This page explains what GDD does ask, and how it groups the questions so that a
change costs you at most two short sittings. Read it if you worry that agents will either flood
you with questions or ship without you.

## Before you start

[GDD in one page](method.md) introduces the gates and the colors. The sittings, the Decision
Inbox and the G4 answers described here are checked by
[`features/sitting.feature`](../../features/sitting.feature),
[`features/sitting-g4.feature`](../../features/sitting-g4.feature) and
[`features/recipe.feature`](../../features/recipe.feature).

## You are the stakeholder, not the code reader

Reviewing every change yourself does not scale. An agent writes code faster than you can read
it, and an agent that waits for you is an agent that is not working. If every change needed
your review, you would be the slowest part of the system, and the review would get worse as you
got tired.

So GDD gives you the job a stakeholder has in a good team. You decide **what** the software must
do, **where** a mistake would be serious, and **whether the result is good enough to release**.
You do not decide whether a loop is correct. The tests, the scenarios, mutation testing and the
Reviewer decide that, and they do not get tired.

Each of your decisions rests on something you can judge without reading code:

| You decide | By looking at | At |
|---|---|---|
| What the change must do | Scenarios in plain sentences | G1 |
| Where a mistake would be serious | Red zones, each with its reason in words | The same sitting |
| How a red change will be built | A plan at plan level: what, how, risks, how to roll back | The same sitting |
| Whether the result is good | The demo in pre and the Reviewer's "How it works" | G4 |

## How many stops

A *stop* is a moment when a change waits for you. The color and the lane of a change decide how
many stops it has, as [`GDD.md`](../../GDD.md#human-stops) lists them:

| Kind of change | Your stops |
|---|---|
| The Setup (G0) | 0: the machines open it |
| Fast Lane | 0 |
| Full Lane | 0 to 2 |
| Red | 2: one sitting (plan, scenarios and zones), then G4 |
| Hotfix | 0 or 1 |

A change also leaves you at most one message. Most changes in a mapped project are yellow
without new scenarios, so they go from launch to production without stopping once.

The mnemonic: *the machines open 0; the human opens 1 and sometimes 4; the machines guard 2
and 3.*

## One sitting per change

A **sitting** is the one moment, before Build, when you approve everything a change needs from
you. The Orchestrator gathers it into one message in the chat, and `gdd sitting CHANGE` shows the
same on one screen:

- the scenarios of the change's branch, with the hidden examples marked as hidden;
- the red zones waiting for your approval, each with its reason, or a note that they are
  approved already;
- for a red change, its plan.

In the same message, the Orchestrator also proposes the line the change will add to the
changelog, so even that costs no extra stop.

You approve once. The screen first shows exactly what it will record, and writes nothing before
you confirm. Then it records G1, the red zones and the plan together. If something is wrong, you
ask for changes in your own words, and gdd records that as a G1 KO with those words.

GDD groups the approvals into one sitting because each approval is an interruption, and an
interruption costs more than the decision. Three questions in one sitting take a few minutes.
Three questions on three different afternoons cost three changes of context.

The sitting comes before Build because changing a scenario after the build started means
approving it again and building again. Deciding first is cheaper than deciding twice.

## The G4 sitting

G4 is the other stop, and only changes that need it reach it: red ones, and those that had new
scenarios. By then the change is already merged and running in pre, the pre-production
environment. The Orchestrator brings it to the chat, and `gdd sitting CHANGE` shows what to judge
it by:

- the demo: video or screenshots of the scenarios or, for a library or a tool without a screen,
  the readable run of its scenarios, with its numbers;
- **How it works**: three to six plain lines the Reviewer writes when it says OK, on where the
  change sits in the code and what pattern it sets, so you understand what you approve without
  reading every line;
- the decisions taken without you;
- the changelog line;
- where the change's time went.

Then you answer, in the chat or on the screen. On the screen, `o` is OK, `k` is KO and `p` is
polish. KO and polish need your words. Before anything runs, the screen shows the exact
`gdd resume` command and what follows.

## Done is not good

A change that passes every gate can still be unfinished. AI reaches something that *looks*
finished very fast, and the last part, what makes it correct and specific, is left undone
because it already "works". GDD calls this principle **done is not good**. It comes from a
Discover session on the problem
([`docs/discover/2026-10-26-done-is-not-good.md`](../discover/2026-10-26-done-is-not-good.md)).

The machines cover half of it. Hidden examples and mutation testing catch tests that lie, and
the Reviewer of another family is the critic. The other half is yours: you are the **editor** at
G4.

That is why G4 has three answers, not two:

- **OK**: promote it.
- **KO**: reject it; production stays as it is.
- **Polish**: one more Builder round with your notes, then the gates and the demo again, and back
  to G4.

Polish is not a failure, and it is not one more stop: it **replaces a KO**. "The button works
but its message says *Error 3*" is a polish note, not a reason to throw the change away.

The quality bar is written down, so that you do not repeat it. Each project has
`docs/standards.md`: what good looks like here, in words, such as tone, naming, error messages
and what you do not want. The Builder reads it before it builds, and the Reviewer judges against
it. The Brief says what and for whom; the standards say how good. gdd's own
[standards](../standards.md) are an example.

## The Decision Inbox

Every change leaves you one message, its **Decision Inbox**, in `.gdd/changes/<id>/inbox.md` and
on `gdd inbox CHANGE`. It has a fixed shape, so you always know where to look:

- **How it works**, from the Reviewer, when the change waits at G4;
- **Needs your decision**: what only you can decide, each with a recommendation and at most one
  alternative;
- **Decided without you**: the reversible calls the agents made on their own, and the paths
  outside the plan that the scope rule accepted;
- **Parked**: why the change stopped, and what to do;
- and, when they apply, your polish notes, the security exceptions the change adds and its known
  debt.

"Decided without you" is what lets the agents keep going. The rule for the Orchestrator and the
Builder is this: if a choice is reversible and you would hesitate between asking and deciding,
decide and write it down. You read the list after the fact, in one place, instead of answering
each question as it comes.

## Learn turns repeated notes into rules

If you write the same polish note twice, the method has failed you once. Phase 5, Learn, exists
to stop that. `gdd learn` lists your polish notes from G4 next to each other, so the
Orchestrator can find the ones that repeat. It also lists the classes of Reviewer findings that
came back in two changes or more.

A repeated note becomes something that stops the problem before it reaches you again: a line of
`docs/standards.md`, a lint rule, a test or a hook. Learn has no gate; it only makes the other
gates stricter. Over time this is what keeps your stops few: the number of human stops per
change is one of the signals [Measuring what matters](measuring.md#gdds-own-signals) follows.

## What was rejected

- **Reviewing code as the human gate.** It does not scale, and it puts the person where machines
  are better.
- **One approval per gate.** More interruptions, the same decisions.
- **Polish as an extra stop after a KO.** It would make "almost right" cost you two rounds
  instead of one.
- **Asking every reversible question.** The agents would wait all day; the Decision Inbox shows
  you what they decided instead.

## See also

- [Approve the scenarios](../how-to/write-scenarios.md): what a good scenario looks like
  at G1.
- [Review a change at G4](../how-to/release-gate.md): OK, KO or polish, step by step.
- [Get a parked change going again](../how-to/parked-change.md): reading the Decision Inbox.
- [Measuring what matters](measuring.md): polish rate and human stops as signals.
