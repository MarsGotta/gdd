# Constitution

What every agent needs at every gate and can't work out from the code. It goes into every call, so keep it short. A rule that matters in only one zone goes to that zone's context in the blueprint.

## The product

Five lines, each marked as seen or as assumed. "Not known yet" is an answer, and it stays in sight until someone knows.

- **Who it is for:** <a person you could name, and what they are trying to do>
- **What happens to them today:** <what they do without it, and what that costs them>
- **What the first version leaves out:** <what waits, as a consequence they can weigh>
- **How we'll know it serves:** <the signal that it helps, and the one that would make us stop>
- **The biggest doubt:** <what would change the plan if it turned out wrong>

## Stack

- <languages, frameworks and their versions>

## Commands

Gates run these same commands, locally and in CI. Each row holds its whole command, never a pointer to another file.

| What | Command |
|---|---|
| Install | `<command>` |
| Build | `<command>` |
| Test | `<command>` |
| Lint | `<command>` |
| Scan for secrets | `<command>` |

With the project's runner, a command reads `sh gdd/check.sh <check id> -- <command>`.

**Use it:** `<command that puts the product in use from a fresh copy>`

## How a change ships

<For example: a pull request to the main branch, which a person merges.>

## Rules that hold everywhere

What must never happen in this product, whichever part a change touches, in the person's words, and then the few rules of how it is built that hold in every part. Three to seven: this file goes into every call, and every brief is checked against each one.

- <One rule that holds everywhere, in the person's words. Repeat for each rule.>
