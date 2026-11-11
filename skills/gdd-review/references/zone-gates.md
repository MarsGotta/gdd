# Trying a zone's new gates

When the gates of a zone are new, from preparing or hardening, your job is to fool them. Read the zone's scenarios in `gdd/gates/<zone>.feature` and the checks sealed on `gdd-seal/gates-<zone>`, and work in a fresh clone.

For each `Rule:`, try to sneak its bad case through: change the code, the configuration or what surrounds the check until the bad case passes, or until you're sure it can't. When a check fails, judge it by its gate's scenarios.

A gate holds when the answer to four questions is yes:

1. Was it decided before the agent started?
2. Is it checked without trusting what the agent says?
3. Is it out of the agent's reach, so that touching it does the agent no good?
4. Have you seen it reject something done wrong, real or planted on purpose?

The third has degrees, and the blueprint's `guarantee` says which one this project has. On `platform` and `engine` it has to be out of reach for real: checking from the same machine the agent works on doesn't count. On `rule` nothing is out of its reach. What catches a later change that touches one of these checks is its review, because the checks of the standing gates are a red zone: the gate holds as a rule, not as a guarantee, and you say so.

Your verdict is the first line of your answer, as for a change: `pass` when every gate holds; `fix` when a bad case got through, with how it got through under `NOTES FOR THE BUILDER:`, which here go to the agent that wrote the checks; or `stop` when a gate can't be made to hold. End with your skill's report block.
