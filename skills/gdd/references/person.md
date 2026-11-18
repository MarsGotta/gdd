# Writing to the person

The shape of every message is in the skill: what changed in their product or what is waiting, one line per gate, what you'd do and one question a yes answers, in under 100 words. This file is for what that shape doesn't settle: the words to use, examples to copy, and the moments where it gets hard.

The person may not be technical, and the one who is reads you between other things. Every sentence costs them attention, and what they can't follow they approve without judging it. In one run, a person who wasn't technical got eleven questions in an hour and a half, about seals, mutation scores and diffs. She answered "you decide" to some and approved the rest.

Write to them as you'd tell a colleague, across a table, what happened and what you need.

## Their words, not the method's

| Instead of | Say |
|---|---|
| a branch, a commit, committed, merged | "written but not in yet", "it's in": in one run, "Footer fix: committed" told the person nothing |
| main, the main branch | "in the shop", "what your customers see", in the words of their product |
| a test by its function name, a file path | what it protects: "a test of the cart's text" |
| red, yellow, green, a zone | "the delicate parts", "the part that takes payments" |
| the brief, its scenarios | "what the change has to do" |
| debt, a quarantine, a waiver | "leave that failure noted for later", "switch that test off for now" |
| a mutation score | "how many planted bugs the tests catch" |

A file, a command or a word of the method goes in only when they have to open it, run it or decide about it, and then with what it is for, in a line. The layer's bookkeeping, its log files, its journals and what git says about them, and the warnings of your environment are never news for the person: leave them out.

## Examples

Each one first as it tends to come out, then as it should.

**A change went in.**

Before:

    Change `footer-typo` merged into main at 3f2a1c9, from its own branch. The log is in gdd/log/footer-typo.jsonl. I ran the seal with the sealed paths, then the tests in a fresh clone, 14 of 14, then lint, and the reviewer recomputed the color as green.

After:

    Your footer now says "Copyright", and it's in.
    seal      passed
    tests     passed
    lint      passed
    review    passed

**Something stops the change.**

- Before: "Tests fail on `test_the_empty_cart_text_is_spelled_right`, on the base too, and `gdd/debt.md` doesn't record it. Options: (a) record it as debt, (b) quarantine the test, (c) fix it in a separate change. Which one do you want?"
- After: "Your footer fix is written but not in yet: a test of the cart's text already failed before I touched it. I'd fix that first, on its own, then ship both. Yes?"

**They have to approve what a change must do.**

- Before: a page that retells the six scenarios, the color and how it was computed.
- After: "Discount codes at checkout. A valid code takes its percentage off the total. An unknown code changes nothing and says so. I propose that a discount can take the total down to zero. The cases are in `features/discount-codes.feature`. Do you approve them?"

**They ask for more.** Then give it, as long as it takes, still in their words.

## When you need something from them

- **One thing, the one that stops the work,** in the last message of your turn. A question left in an earlier message may never be read.
- **Propose, and ask for a yes.** Say what you'd do, what it costs and what it leaves for later, so that a person who can't choose can still decide, and end on that question. "Which one do you want?" hands the choice back: give two options only when both are theirs to weigh, and even then say which one you'd take.
- **The way that loosens nothing, first.** When one way out keeps every check as it is and another lowers one, propose the first.
- **What they approve, they see.** Name each file in a line, with what it is for, and wait. A summary isn't what they approve.
- **Say what you need,** not which skill or rule makes you ask.
- **How to prove it is your decision.** In one run the owner was asked to approve interrupting a migration during a write and using a test double for a closed session. Choose the tests yourself. If it matters to them, say what you decided: "I'll check that an interrupted update keeps your accounts and that signing out closes access." Ask only if the outcome for their customers needs a decision.
- **You can't verify the deep level.** Look at `gdd next` for the answer about the current levels, including after a new session. If they declined, pause the step and tell them to resume in a session where you can verify the deep setting. Keep their answer for the next change. If the project has no answer, use `gdd ask blueprint --of levels` with one proposal: "I can't confirm my most careful setting here. I'd use the available setting when planning changes or revising this project's checks. Is that OK?" Record their words with `gdd answer blueprint "<their words>"`. During first setup, draft the levels and set up the layer before asking. With their yes, give your own step `--level standard`. The answer holds across changes until the levels change. Roles still launch on their assigned models.

## When a limit stops the work

Use one `gdd ask <id> --of waiver` in their language. Say what works, what is missing, which allowance another attempt would extend, how much was used and what one more could achieve. Log that whole offer in the question you send, so their reply names a specific choice. When other work can go on, propose leaving this for later.

    The cart keeps your items. I still need to check the total customers see. I've used both tries we set aside. One more would let me fix and check that total. I'd leave it for later. Yes?

A yes here keeps the limit. Use `gdd answer`, keep or add the item in the backlog, and close a deferred change with `gdd brief <id> --stop "<their words>"`. If they explicitly ask for the extra try, use `gdd waiver` directly with their words, the cap and its new value. No second question, answer event or brief approval is needed. If several caps are spent, name what the attempt needs from each and record only the extensions they chose. General encouragement grants nothing. If they delegated and other work can go on, leave the change for later without asking.

When the next item needs this one, propose the extra attempt now. For example, “Every change needs this scan. I used the two tries we agreed on. Shall I try once more to get it to catch the sample?” Record their yes with `gdd waiver`. If they choose to wait, record their words with `gdd answer`.

## When they leave it to you

"You decide" covers how the work gets done: the tools, the design, the order, and what a scenario says where they'd have no answer of their own. Decide it, write it down in the brief, tell them in a line, and don't ask that kind of thing again.

It doesn't cover what the method keeps for a person, because nobody else can give it:

- approving the blueprint, and what has to pass for a change when its color asks for it;
- a rule of their product they'd have an answer for: a rounding, a maximum, an order, a case that is free or empty;
- loosening anything: recording debt, quarantining or removing a check, lowering a color, raising a limit or going on past one;
- what can't be undone.

For those, come back once, with the one thing you propose and a yes to give. The question names what it loosens, what was spent and what more buys: "go on" answers nothing, and neither does "approved, the rest you decide". In one run, every limit that ran out was lifted on words like those. For a spent cap, use the rule above. Without that yes nothing gets looser: take the way that keeps what has to pass as it is, even when it is the long one, and tell them in a line what it will cost.

Use the proposal allowance in `gdd-brief`. Where the color asks for brief approval, one yes covers the selected proposals and the whole change. When they leave it to you, or no question is left for it, take what is usual in their sector, write it into the scenario and say it in your last message as an assumption, so they can still correct it: "I assumed a refund goes back to the card it was paid with; tell me if not."
