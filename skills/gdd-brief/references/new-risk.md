# When a change brings a risk for the first time

Open this when the change does something the product never did and no gate of its zone covers it: the first data it stores, the first money it moves, the first call to a service outside. The recipe of its area, in `../gdd-recipes/`, lists the first times that usually need a gate of their own there. A gate that arrives before the change that needs it is the point of growing the gates as the project goes.

1. **Write no check in the change.** What a new risk needs is a gate of the zone, which changes the blueprint, and the person approves it. A test you add to this change would judge only this change, and the next one would go without it.
2. **Propose the gate, once,** in the same question as the brief's approval when the color asks for one, or in a question of its own: what can go wrong now that couldn't before, the gate that would catch it in a sentence, and the one you'd take. Its rules go in `gdd/gates/<zone>.feature` through `gdd-prepare`, and its item in the backlog, `gates-<zone>`.
3. **With their yes,** brief the change with `--waits-for gates-<zone>`. It is briefed, checked and built meanwhile, and its gates wait until that item is done, then take in what the main branch got, as the layer names.
4. **If they'd rather go on without it,** raise the color to what that risk asks, write under "What it won't do yet" that no gate covers it yet, and add the item to the backlog anyway; a change already briefed with `--waits-for` takes it back in a new version with `--waits-for none`. That is their call, and you say it in one line.

What is red is never left for later: a first time that can't be undone, or that a norm of their sector covers, gets its gate before the change, and the person decides only how.
