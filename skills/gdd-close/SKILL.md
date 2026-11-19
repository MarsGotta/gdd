---
name: gdd-close
description: Closes a change of a Gate-Driven Development (GDD) project once it is in use, by gathering what can be seen against what its brief expected and recording the person's verdict, a yes, a not yet or a no. Loaded by the GDD orchestrator when the person asks to look at a change in use or says whether it served. Not for a change that hasn't shipped, and never to give the verdict itself.
license: MIT
metadata:
  version: "0.16.0"
  contract: gdd/1
---

# Closing a change

Getting through its gates leaves a change built. It is closed when someone has looked at whether it served, against what its brief said before anyone built it. The agent says "it served", and something else checks if that is true: you gather what can be seen, and the verdict is the person's, never yours. You do it yourself, as the orchestrator, in your conversation with the person.

It never starts by itself. A change in use waits in sight, however long, and stops nothing: no deadline closes it, nothing cancels it, and the next request doesn't wait for it. `gdd next` lists what waits. Say it in one line of your summary, with no question, and start only when the person asks to look at one or tells you whether it served.

In every command below, run the project's `.gdd/gdd` first. Use `gdd` on the path only if `.gdd/gdd` is unavailable and `gdd --version` names the GDD layer.

## What was expected

Run `gdd close <id>`. It writes nothing. It prints the three lines the person approved with the brief (what was expected, the signal that would show it and where to look), how long the change has been in use and any earlier look. Use them as they are: never reword them into something easier to meet, and never add a signal the brief didn't name, because a signal chosen once the result is in sight gets read in its favor.

## Gathering what is seen

Go to the place the brief named and read the signal there.

- **From where the builder couldn't write it.** Never the builder's report, the reviewer's, the change's own tests or gates, or anything the change itself writes about how it is doing. Those say it was built as written, not that it served.
- **Each fact with its source and its date.** A number with no source and no date is a claim.
- **What you can't see, said.** If you can't reach that place, or the signal isn't there yet, say so and ask the person to look. Never put another source, an estimate or what should have happened in its place.
- **Read only.** Change nothing where you look.

## Asking

Record the question first, with the `gdd ask <id> --of close` that `gdd close <id>` printed. Then send one message under 100 words, in the person's language and in their product's words:

    <What was expected, as their brief said it.>
    <What is seen, each fact with where it comes from and its date.>
    <What couldn't be seen, if anything.>
    <Did it serve: yes, not yet or no?>

"You expected reminders to cut missed appointments. The March calendar shows 4 missed of 60, and February's 11 of 58. I couldn't see who turned reminders off. Did it serve: yes, not yet or no?"

Propose no verdict and don't lean the facts toward one. This is the one question you send without a proposal, because its answer is the verdict. Leave out how the build went.

## The verdict

Only the person's own words are a verdict, or those of whoever the blueprint names. Silence, "go on", thanks or an answer about something else are not one: the change keeps waiting. If they say "you decide", tell them in one line that this one is theirs, and leave it waiting.

Record it at once, in their words, with the command the layer printed: `gdd close <id> --verdict <yes, not-yet or no> --who <person> --where <conversation, commit or pull request> --saw "<what they saw>"`.

- **Yes.** It is closed. Nothing else to do.
- **Not yet.** It stays in use and in sight. Don't ask again until they come back to it.
- **No.** It is closed too, and its log is not opened again. The layer prints two ways out, and you propose the one you'd take, for a yes. If it works as written and didn't serve: a new request in the backlog, or nothing if they accept it as it is. If it broke a rule or did harm: the fix is a normal change through `gdd-brief`, and a hardening proposal through `gdd-harden`, because a gate should have caught it. Add `--then <the backlog item, harden or none>` when they already said which.

A verdict you recorded wrong is corrected with `gdd correct <id> --line <n> --void "<what was wrong>"`, and the change waits again.

## What promised nothing

A change whose brief says `Nothing to close` closes by itself once the days in the blueprint's `closing` go by with nothing recorded against it. Nobody is asked. If a hardening record names it before that, it waits for the person's verdict like the rest.
