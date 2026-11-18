# The message that asks for the approval

Copy it, fill it in and send nothing else: under 200 words, in the person's language and in their words. The `gdd` skill's reference on writing to the person has the words to use.

    Your project is ready for you to approve. Nothing is final and nothing gets built until you do.

    Please read:
    - `gdd/blueprint.md`: <what it is for, in a line>
    - `gdd/constitution.md`: <the same>
    - <each other file you wrote, the ones that go together in one line>

    Before you say yes:
    - <How many recent changes or planned items came out green, yellow and red. With history, where the fixes fell.>
    - <What the checks would miss, including any area no zone covers, or that the sample revealed no gaps.>
    - <What protects nothing yet, anything within an agent's reach and anything you couldn't confirm.>
    - <What isn't set up or tried yet, and when it will be. With no code yet, use the line from `references/new-project.md`.>

    Do you approve them as they are?

Keep "Before you say yes" to four lines at most. Use two for the trial of the blueprint. Combine other caveats that belong together, including anything that comes from memory because you couldn't look it up. What they asked for by name gets its answer in the line of the file that holds it. No other headings and no other lists.

Filled in, for a shop:

    Your shop's plan is ready to read. Work starts after your approval.

    Please read:
    - `gdd/blueprint.md`: what each change must pass. It includes the mutation testing you requested for delicate changes.
    - `gdd/constitution.md`: what must never happen, like selling out-of-stock goods or storing a card number.
    - `gdd/gates/`: what each check must prove. Please correct any payment rule that doesn't fit.
    - `gdd/backlog.md`: the first jobs, in order.

    Before you say yes:
    - Of your last 25 changes, 10 came out green, 9 yellow and 6 red, including both payment fixes.
    - The tests would have missed changes to order processing alone. The revised plan covers them.
    - The card rules come from the card schemes' standard. The rest are my proposals. None of it is legal advice.
    - The rules depend on agents following them until the platform enforces them. Setting up and trying the checks comes first after your approval.

    Do you approve these files as they are?

What stays out, because it is in the files: the full list of zones and their colors, the models, the tools, the limits, what an emergency may skip and how you got here. If they ask about any of it, answer then. That holds when the person left the rest to you: in one run the message added a list headed "What I decided for you", with the levels of risk, the models and the checks, and came to 310 words.

For a person who isn't technical, say what a file is for and give its name only where they have to open it, and use no word of a tool, a model or git: "the checks every change passes" for a linter or mutation testing, "who has to approve a change" for the file of owners, "ready" for committed. In one run, the message named the file of owners, a linter, a mutation tool, the models and "committed".

Before you send it, count its words with a command, such as `wc -w` on a here-document that holds it: over 200, cut it and count again, and send it only once it is under. In one run that counted this way it came to 195 words, and in one that didn't, to 428. Then reread it as someone who never saw the method.
