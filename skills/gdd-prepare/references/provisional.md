# A color still provisional

Open this when a change needs a color the blueprint still has under `provisional`: `gdd color --paths` or `gdd brief` said so. The first yes covered what is costly to get wrong, and left what a change of this color goes through for when the first one came. It has come, so the person decides it now, before its brief.

1. **One question, with what you propose.** In their words, say what a change of this color would go through as the blueprint has it: whether they approve its cases before anything is built, whether a person looks at it at the end, how long it may take and how many tries it gets. Then what you'd change, if the changes already done say anything, or that you'd keep it. Never a list of check ids or settings. Record it with `gdd ask blueprint --of blueprint "<the question, in a line>"`, apart from the change's own questions: one at a time.
2. **Their answer goes into the blueprint.** A yes keeps the proposal, and a correction changes it. Either way, write what they decided into that color's `checks`, `level` and `limits`, and take the color out of `provisional`. "You decide" is a yes to what you proposed: say so in one line.
3. **Then as any change to the blueprint.** They approve it, with `gdd approve blueprint --of blueprint`, and it reaches the main branch before the change is briefed, because the color is computed from there. Then go on with the change where it stopped.

What a red change goes through is never provisional: what can't be undone is decided with the first yes.

    This is the first change of medium risk. Until now, what it goes through was my proposal: you approve its cases before anything is built, a second assistant reviews it, and it can take up to 45 minutes and three tries. I'd keep it that way. Shall I?
