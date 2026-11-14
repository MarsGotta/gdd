# The message that asks for the approval

Copy it, fill it in and send nothing else: under 200 words, in the person's language and in their words. The `gdd` skill's reference on writing to the person has the words to use.

    Your project is ready for you to approve. Nothing is final and nothing gets built until you do.

    Please read:
    - `gdd/blueprint.md`: <what it is for, in a line>
    - `gdd/constitution.md`: <the same>
    - <each other file you wrote, the ones that go together in one line>

    Before you say yes:
    - <What protects nothing yet, or what you found within an agent's reach.>
    - <What you couldn't look up or confirm, and so comes from memory.>
    - <What isn't set up or tried yet, and when it will be. With no code yet, the line `references/new-project.md` gives.>

    Do you approve them as they are?

Everything this skill tells you to say to the person before the approval goes under "Before you say yes": four lines at most, a sentence each, the one that weighs most first, and none when there is nothing to say. What they asked for by name gets its answer in the line of the file that holds it. No other headings and no other lists.

Filled in, for a shop:

    Your shop is ready for you to approve. Nothing is final and nothing gets built until you do.

    Please read:
    - `gdd/blueprint.md`: which parts of the shop are delicate, and what every change has to pass before it goes in. The mutation testing you asked for is there, on the delicate changes.
    - `gdd/constitution.md`: what must never happen in the shop, such as selling what isn't in stock or a card number touching it. Put them in your own words.
    - `gdd/gates/`: three short pages, one for each thing a change has to prove.
    - `gdd/backlog.md`: the first work, in order.

    Before you say yes:
    - Nothing stops an agent from breaking these rules yet: without a platform they hold only if the agents follow them.
    - The rules on card payments come from the card schemes' own standard and the rest are my defaults. None of it is legal advice.
    - The checks are written but not set up: that is the first thing I'll do after your yes.

    Do you approve them as they are?

What stays out, because it is in the files: the zones and their colors, the models, the tools, the limits, what an emergency may skip and how you got here. If they ask about any of it, answer then. That holds when the person left the rest to you: in one run the message added a list headed "What I decided for you", with the levels of risk, the models and the checks, and came to 310 words.

For a person who isn't technical, say what a file is for and give its name only where they have to open it, and use no word of a tool, a model or git: "the checks every change passes" for a linter or mutation testing, "who has to approve a change" for the file of owners, "ready" for committed. In one run, the message named the file of owners, a linter, a mutation tool, the models and "committed".

Before you send it, count its words with a command, such as `wc -w` on a here-document that holds it: over 200, cut it and count again, and send it only once it is under. In one run that counted this way it came to 195 words, and in one that didn't, to 428. Then reread it as someone who never saw the method.
