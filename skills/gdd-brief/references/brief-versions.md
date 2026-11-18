# Revising a logged brief or taking back a change

Once its hashes are in the log, a brief changes only as a new version: the same file with `version` raised by one, sealed and logged the same way, with `gdd seal` and `gdd brief`. It goes back to the person only if its color asks for `brief-approval` and what has to pass changes. A waiver or a change to the list of paths keeps the same approval when its rules and scenarios still stand. If the person edits their copy, that's a correction: compare it with the seal and make a new version.

## When a change comes back

- **An agent stopped.** Check its reason against the repository, then write a new version: add the missing path, change the scenario it contradicts (with the person's approval, when the color asks for it), or drop the change. Never loosen a check so that it fits.
- **Its color rose.** Keep its id. Write the brief of the new color, or raise the one it had, from the request and the blueprint, as if nothing had been built: don't read what the builder did. With its code in sight you'd write what it did, not what was needed.

A change you drop ends in a `brief` step whose `result` is `stopped`: `gdd brief <id> --stop "<why>"`.
