# Revising a logged brief or taking back a change

Once its hashes are in the log, a brief changes only as a new version: the same file with `version` raised by one, sealed and logged the same way, with `gdd seal` and `gdd brief`. It goes back to the person only if its color asks for `brief-approval` and what has to pass changes. A waiver or a change to the list of paths keeps the same approval when its rules and scenarios still stand. If the person edits their copy, that's a correction: compare it with the seal and make a new version.

## When a change comes back

- **An agent stopped.** Check its reason against the repository, then write a new version: add the missing path, change the scenario it contradicts (with the person's approval, when the color asks for it), or drop the change. Never loosen a check so that it fits.
- **Or cut it there, when building taught something.** When the builder stopped because a scenario can't be true as written, from what it found in the product, and not for a path or a check, offer a cut too: what it built for the other scenarios ships as a part once its gates pass, and what it learned goes to the backlog as an item of its own. Offer it only if that part leaves the product working, can be undone on its own and gives the person something to use or see, in the same question as the new version, with the one you'd take (`--of decision`). With their yes:
  - a new version without the scenarios the stop is about, whose closing says `Nothing to close: a part of <the request>, which <item> closes`, and which goes back to the person when its color asks for it. Its color stays: cutting doesn't lower the risk;
  - `gdd backlog add <item>` with what the stop taught, in the person's words;
  - the new version goes through its checks and gates like any other: the checks role takes the tests of those scenarios out of the seal, and whatever the builder had started for them comes out, or waits behind a switch that is off.

  The item is briefed later from the request and what the stop taught, never from the builder's code, and its checks are written, as always, by an agent that never saw that build. Never cut to fit a cap, and never on your own.
- **Its color rose.** Keep its id. Write the brief of the new color, or raise the one it had, from the request and the blueprint, as if nothing had been built: don't read what the builder did. With its code in sight you'd write what it did, not what was needed.

A change you drop ends in a `brief` step whose `result` is `stopped`: `gdd brief <id> --stop "<why>"`.
