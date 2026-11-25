# The message that asks for the approval

It is often the first long message the person gets from you, and the one where they decide whether to trust the plan. Write it for them, in their language and their words, as the `gdd` skill's reference on writing to the person says: whole sentences, never notes. Keep it under 250 words: if it doesn't fit, leave out what is in the files, never the words that join the ideas.

Their yes covers what is costly to get wrong: the red zones, with what must never happen there, what an urgent change may skip and who says whether a change helped. What a change of lower risk goes through is `provisional` in the blueprint, and you ask it when the first one comes, as `references/provisional.md` says: here it gets half a sentence in line 4.

    Your project is ready for you to approve. Nothing is final and nothing gets built until you do. <One sentence that tells the plan itself: what gets the most care and why.>

    Please read:
    - `gdd/blueprint.md`: <what it is for, in a sentence, with what an urgent change may skip and who says whether a change helped>
    - `gdd/constitution.md`: <the rules of their product, one of them quoted>. Put them in your own words.
    - `gdd/gates/`: what the checks must prove. Please correct any <red zone> rule that doesn't fit.
    - <each other file you wrote, the ones that go together in one line>

    Before you say yes:
    - <How many recent changes or planned ones needed each level of care. Name the parts of the product and where past fixes fell.>
    - <What the checks would miss. Count the changes that touch an area no zone covers and name it. Say what the revised plan covers or that the sample revealed no gaps.>
    - <What protects nothing yet, anything within an agent's reach and anything you couldn't confirm.>
    - <What isn't set up or tried yet, which the first piece of their product can't go in without, though it starts alongside: roughly how long it takes, said as a guess, and how many more yeses you'll ask for, including what a change of lower risk goes through, asked when the first one comes. With no code yet, use the line from `references/new-project.md`.>

    <One question a yes answers that approves these files as they are.>

A person who gets a wall of file names and caveats skips it and approves blind. So the plan comes first, in one plain sentence, and lines 3 and 4 are things they can picture, each with what it means for them or what you'll do about it. What they can do nothing about, and that changes no decision of theirs, stays in the files.

Everything this skill tells you to say to the person before the approval goes under "Before you say yes", in those four lines and in that order, each one or two whole sentences on its own line. Lines 1 and 2 are the trial of the blueprint: each stays on its own line, also when it found nothing, and carries nothing else. Line 4 always says what isn't set up or tried yet. Any other caveat, including anything that comes from memory because you couldn't look it up, joins line 3 or line 4: it never replaces a line and never adds a fifth. What they asked for by name, and what you decided about it, gets its answer in the line of the file that holds it. No other headings and no other lists.

Filled in for a birdwatching diary:

    Your plan is ready, and nothing gets built until you say yes. In short: whatever touches who can see a sighting gets the most care, with its own tests and a second assistant that tries to break it.

    Please read:
    - `gdd/blueprint.md`: how carefully a change to sharing is checked, with the review you asked for. An urgent fix may skip the slow tests, never the sharing ones, and you judge whether each change helped.
    - `gdd/constitution.md`: the rules that always hold, such as "Only you can see a sighting you marked private." Put them in your own words.
    - `gdd/gates/`: what the automatic tests of every change must prove. Correct any sharing rule that doesn't fit.
    - `gdd/backlog.md`: the first changes, in order.

    Before you say yes:
    - Of your last 25 changes, 10 changed labels, 9 changed how you record sightings and 6 changed who can see them, including both privacy fixes.
    - Two of them touched sharing settings the plan had missed, and the revised plan tests those too.
    - The privacy rule comes from your guidance; the other rules are my proposals.
    - Nothing yet stops an assistant from weakening those tests, so I lock them first and check the lock holds, maybe an hour and one more yes from you, while I start on the diary: nothing goes in before it holds. I'll ask what a label goes through when the first one comes.

    Do you approve these files as they are?

What stays out, because it is in the files: the full list of zones and their colors, the models, the tools, the limits and how you got here. So does what a change of lower risk goes through, but for its half sentence in line 4: you ask it later. If they ask about any of it, answer then. That holds when the person left the rest to you.

For a person who isn't technical, say what a file is for and give its name only where they have to open it, and use no word of a tool, a model, git or the place you run in: "the checks every change passes" for a linter or mutation testing, "the check that no password gets in" for a secret scan, "who has to approve a change" for the file of owners, "ready" for committed, "one past change" for one commit, "on this computer" for a sandbox. Their four lines, for the same diary:

    - Your diary has only one past change, too few to test the plan on, so I went by what each part does: sharing gets the most care and labels the least.
    - Every part of the diary is covered by the plan.
    - The privacy rule comes from your guidance; the other rules are my proposals.
    - Nothing yet stops an assistant from weakening the tests every change must pass, and I couldn't try one of them on this computer. Locking and trying them all is the first thing I do after your yes, maybe an hour, while I start on what you'll use: nothing goes in before they hold. I'll ask what a label goes through when the first one comes.

Before you send it, read it as someone who never saw the method. If a sentence would puzzle them, say it another way, even if the message gets a little longer.
