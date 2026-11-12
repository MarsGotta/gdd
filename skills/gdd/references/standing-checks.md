# Setting up the standing checks

The standing checks are the machine checks the blueprint's `checks` name. They hold for the whole project, so they get set up and tried once, never once for each zone, and no change starts before they hold. This is the second part of preparing: the blueprint defined them, and here they get built.

This is yours to start. When the item is still `todo` and someone asks for a change, do this first and then go on with the change, in the same turn: don't ask the person whether to, and never offer to go on without it. Write each step to its log when it closes, with the time it closed: a log written whole at the end says nothing about the order. They already said yes when they approved the blueprint, so it has no brief and waits for no approval but the one its color asks for when it ships.

1. **The item.** The backlog item is `standing-checks`, its log `gdd/log/standing-checks.jsonl` and its seal `gdd-seal/standing-checks`, cut from the main branch. It has no brief, and its color is the one its paths give: the files of the checks, of CI and of each tool's configuration.
2. **Setting them up.** Launch `gdd-checks` at the level of that color, with the blueprint and the constitution. It sets each check up, with a tool that already exists before a script of the project's own, proves it once and seals it. Write its `checks` step.
3. **The trial.** Launch `gdd-review` on that seal. It tries each check three ways, once a round: the check rejects a planted bad case, it accepts without it, and a change can't switch it off. When the blueprint names its gates, it also reads each gate's feature against the blueprint, which has to ask for what the feature says. Write its `review` step.
   - `fix`: back to `gdd-checks`, and then tried again. The blueprint's `limits` count as for a change: when the rounds run out, stop and ask the person.
   - `stop`: tell the person. Either a check can't be made to hold, or its files fall in a zone whose color asks for no `review`, which is a pathspec for them to approve.
4. **What it leaves for later.** Tell the person what the trial left under `LATER:` and put it in the backlog, without waiting: it holds no change back. What they approved stays as it is unless they change it. You never widen what has to pass, not even when they leave it to you.
5. **Shipping.** When the trial passes, it ships the way the constitution says, as a change of that color with no brief: the person reviews it where the color asks for `person-review`, and a `ship` step closes its log.

## Later on

These go the same way, each as a backlog item with a log and a seal of its own name:

- **A check the project hadn't set up,** which the color of a new zone asks for: `standing-checks-<check id>`. No change starts in that zone before it holds.
- **A zone's own gate,** which hardening gave it or the person asked for when preparing: `gates-<zone>`, or `gates-<zone>-2` when the zone already has one. `gdd-checks` builds its check from the scenarios in `gdd/gates/<zone>.feature`, and no change starts in that zone before it holds.

A project prepared before version 0.3 has an item `gates-<zone>` for each zone instead. Those that are `done` stand for `standing-checks`, and each zone keeps the gates it has. Where one is still `todo`, ask the person whether to finish it, or to take that zone's `gates` out of the blueprint and set up `standing-checks`.
