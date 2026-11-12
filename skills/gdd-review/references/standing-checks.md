# Trying new standing checks

When standing checks are new, from preparing or hardening, your job is to see each one close, once in each round. Read the blueprint's `checks` and what was sealed on the ref you were given, the features of the project's gates in `gdd/gates/`, when the blueprint names any, and, when a zone got a gate of its own, its scenarios in `gdd/gates/<zone>.feature`. Work in a fresh clone.

Try each machine check three ways, and no more:

1. **It rejects.** Plant its bad case and watch the check fail and name the file and the line. The bad case is the one the blueprint gives it in `breaks` and its gate's feature describes under `Rule: It closes` or, for a check with neither, the plainest thing it exists to catch.
2. **It accepts.** Take the bad case out and watch the check pass.
3. **A change can't switch it off.** Try what a commit can carry: take the check out of CI, empty its configuration, add an entry that makes it skip a file. Then compute the color of that commit from its paths alone, without judgment: a path no zone covers is never green. The check holds when the checks of that color include `review`, or when the platform refuses the commit.

Then, when the blueprint names its `gates`, read each gate's feature against it. A blueprint without `gates` was written before there were features: there is nothing to compare, and that isn't a finding. Under `Rule: When it asks for what`, the blueprint's `colors` and each check's `when` have to ask for exactly the checks its scenarios say, for a change like that one. Where the feature and the blueprint disagree, that is a `fix`: say which of the two the other contradicts, because which one is right is the person's to say.

You try the gate, not the tool. What a mature tool might fail to detect is for its maintainers, until something real gets past it, which is for hardening. What a check of the project's own misses beyond its scenarios changes what has to pass, which the person decides. Neither is a reason for `fix`: name it under `LATER:` and say it's theirs to decide. The machine the check runs on isn't part of a change either (its environment, the tools on its path, the clone's own git files): whether the agent can reach it is the third question below. In one run, reviews that went further kept finding one more way for more than two and a half hours, and no change started.

A secret never goes into your answer, not even the one you planted: say what kind it was and where it was. Your answer reaches a log that is never rewritten, and the secret scan reads it.

A gate holds when the answer to four questions is yes:

1. Was it decided before the agent started?
2. Is it checked without trusting what the agent says?
3. Is it out of the agent's reach, so that touching it does the agent no good?
4. Have you seen it reject something done wrong, real or planted on purpose?

The third has degrees, and the blueprint's `guarantee` says which one this project has. On `platform` and `engine` it has to be out of reach for real: checking from the same machine the agent works on doesn't count. On `rule` nothing is out of its reach. What catches a later change that touches one of these checks is its review, because their files are in a zone whose color asks for `review`: the gate holds as a rule, not as a guarantee, and you say so. What only a platform or an engine could stop, such as an approval forged on the same machine or a push straight to the main branch, is that limit, not a reason for `fix`.

Your verdict is the first line of your answer, as for a change: `pass` when every check holds; `fix` when one of the three tries failed, with how under `NOTES FOR THE BUILDER:`, which here go to the agent that set the checks up; or `stop` when a check can't be made to hold, or when the third try fails because a check's files fall in a zone whose color asks for no `review`: that is a pathspec for the person, not a `fix`, so name the paths. End with your skill's report block.
