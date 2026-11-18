# The message that asks for the approval

Copy it, fill it in and send nothing else: under 200 words, in the person's language and in their words. The `gdd` skill's reference on writing to the person has the words to use.

    Your project is ready for you to approve. Nothing is final and nothing gets built until you do.

    Please read:
    - `gdd/blueprint.md`: <what it is for, in a line>
    - `gdd/constitution.md`: <the rules of their product, for them to put in their own words>
    - <each other file you wrote, the ones that go together in one line>

    Before you say yes:
    - <How many recent changes or planned items needed each level of care. Name the parts of the product and where past fixes fell.>
    - <What the checks would miss, including any area no zone covers, or that the sample revealed no gaps.>
    - <What protects nothing yet, anything within an agent's reach and anything you couldn't confirm.>
    - <What isn't set up or tried yet, and when it will be. With no code yet, use the line from `references/new-project.md`.>

    Do you approve them as they are?

Everything this skill tells you to say to the person before the approval goes under "Before you say yes". Keep it to four lines at most. Use two for the trial of the blueprint. Combine other caveats that belong together, including anything that comes from memory because you couldn't look it up. What they asked for by name gets its answer in the line of the file that holds it. No other headings and no other lists.

Example:

    Your plan is ready to read. Work starts after your approval.

    Please read:
    - <plan>: how each change is checked, including the extra review you requested.
    - <rules>: what must never go wrong. Please correct anything that doesn't fit.

    Before you say yes:
    - Of <n> changes, <n> needed a quick check, <n> a closer look and <n> your review. The fixes were in <part of the product>.
    - Changes to who could see an entry would have missed the tests. The revised plan covers them.
    - The privacy rule comes from the guidance you supplied. The other rules are my proposals.
    - For now the agents can change the checks. Setting up and trying them comes first after approval.

    Do you approve these files as they are?

What stays out, because it is in the files: the full list of zones and their colors, the models, the tools, the limits, what an emergency may skip and how you got here. If they ask about any of it, answer then. That holds when the person left the rest to you.

For a person who isn't technical, say what a file is for and give its name only where they have to open it, and use no word of a tool, a model or git: "the checks every change passes" for a linter or mutation testing, "who has to approve a change" for the file of owners, "ready" for committed.

Before you send it, count its words with a command, such as `wc -w` on a here-document that holds it: over 200, cut it and count again, and send it only once it is under. Then reread it as someone who never saw the method.
