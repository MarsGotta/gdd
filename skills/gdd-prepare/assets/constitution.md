# Constitution

What every agent needs at every gate and can't work out from the code. It goes into every call, so keep it short. A rule that matters in only one zone goes to that zone's context in the blueprint.

## Stack

- <languages, frameworks and their versions>

## Commands

Gates run these same commands, locally and in CI.

| What | Command |
|---|---|
| Install | `<command>` |
| Build | `<command>` |
| Test | `<command>` |
| Lint | `<command>` |
| Scan for secrets | `<command>` |

With the project's runner, a command reads `sh gdd/check.sh <check id> -- <command>`.

## How a change ships

<For example: a pull request to the main branch, which a person merges.>

## Rules that hold everywhere

What must never happen in this product, whichever part a change touches, in the person's words, and then the few rules of how it is built that hold in every part. Three to seven: this file goes into every call, and every brief is checked against each one.

- <One rule that holds everywhere, in the person's words. Repeat for each rule.>

For example, for an alarm clock:

- A disabled alarm never rings.
- Snoozing one alarm never changes another.
- A weekday alarm stays silent on weekends.
