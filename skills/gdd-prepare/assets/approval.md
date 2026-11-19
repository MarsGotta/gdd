# The message that asks for the approval

Copy it, fill it in and send nothing else: under 200 words, in the person's language and in their words. The `gdd` skill's reference on writing to the person has the words to use.

    Your project is ready for you to approve. Nothing is final and nothing gets built until you do.

    Please read:
    - `gdd/blueprint.md`: <what it is for, in a line>
    - `gdd/constitution.md`: <the rules of their product, one of them quoted>. Put them in your own words.
    - `gdd/gates/`: what the checks must prove. Please correct any <red zone> rule that doesn't fit.
    - <each other file you wrote, the ones that go together in one line>

    Before you say yes:
    - <How many recent changes or planned items needed each level of care. Name the parts of the product and where past fixes fell.>
    - <What the checks would miss. Count the changes that touch an area no zone covers and name it. Say what the revised plan covers or that the sample revealed no gaps.>
    - <What protects nothing yet, anything within an agent's reach and anything you couldn't confirm.>
    - <What isn't set up or tried yet, and when it will be. With no code yet, use the line from `references/new-project.md`.>

    Do you approve them as they are?

Everything this skill tells you to say to the person before the approval goes under "Before you say yes", in the template's four lines and in that order. Lines 1 and 2 are the trial of the blueprint: each stays on its own line, also when it found nothing, and carries nothing else. Line 4 always says what isn't set up or tried yet. Any other caveat, including anything that comes from memory because you couldn't look it up, joins line 3 or line 4: it never replaces a line and never adds a fifth. What they asked for by name, and what you decided about it, gets its answer in the line of the file that holds it. No other headings and no other lists.

Filled in for a birdwatching diary:

    Your plan is ready to read. Work starts after your approval.

    Please read:
    - `gdd/blueprint.md`: how each change is checked, including the extra review you requested for sharing.
    - `gdd/constitution.md`: rules such as "Only you can see a sighting you marked private." Put them in your own words.
    - `gdd/gates/`: what the checks must prove. Please correct any sharing rule that doesn't fit.
    - `gdd/backlog.md`: the first jobs, in order.

    Before you say yes:
    - Of your last 25 changes, 10 changed labels, 9 changed how you record sightings and 6 changed who can see them, including both privacy fixes.
    - Two touched sharing settings that the plan missed. The revised plan runs the privacy tests there too.
    - The privacy rule comes from the guidance you supplied. The other rules are my proposals.
    - For now the agents can change the checks. Setting up and trying them comes first after approval.

    Do you approve these files as they are?

What stays out, because it is in the files: the full list of zones and their colors, the models, the tools, the limits, what an emergency may skip and how you got here. If they ask about any of it, answer then. That holds when the person left the rest to you.

For a person who isn't technical, say what a file is for and give its name only where they have to open it, and use no word of a tool, a model, git or the place you run in: "the checks every change passes" for a linter or mutation testing, "the check that no password gets in" for a secret scan, "who has to approve a change" for the file of owners, "ready" for committed, "one past change" for one commit, "from here" for a sandbox. Their four lines, for the same diary:

    - Your diary has one past change, too few to try the plan on. Sharing gets the most care and labels the least.
    - Nothing in it falls outside the plan.
    - The privacy rule comes from the guidance you gave me. The other rules are my own proposals.
    - Nothing stops an assistant from changing the checks yet, and I couldn't try one of them from here. Trying them all is the first job after your yes.

Before you send it, count its words with a command, such as `wc -w` on a here-document that holds it: over 200, cut it and count again, and send it only once it is under. Then reread it as someone who never saw the method.
