# Exploring a zone on purpose

Open this when the backlog holds `explore-<zone>`, which the layer adds once the blueprint's `harden.explore_after` changes touched a zone since it was last explored, or when the person asks for an exploration. Every check of GDD comes from a brief, and what no brief ever wrote only someone looking without a script finds. An exploration finds the gates that are missing: it is no gate, and it stops nothing.

1. **A charter, in one sentence.** Explore <the zone, or one flow of it> with <what you use: data, a device, a slow network, two people at once> to discover <what you want to know>. Choose it from what the zone does and from what got through lately, never from a list.
2. **Who explores.** The person, or an agent with a clean context that runs the product, as `gdd-review` runs a change: the charter, the constitution's `Use it` command and the zone's own rules, never the briefs, the scenarios or the tests, which would make it look where everything was already looked at. It uses the product as it is in use, with data that can be believed and never with credentials of production.
3. **What comes back is findings,** each with how to see it again, never a pass or a fail. A finding that something shipped does wrong is a fix, briefed with `--fixes` the change that shipped it. One that shows a risk no gate covers is a proposal of a gate, with `gdd-prepare`. One that matters to nobody is dropped, and you say why.
4. **Record it** with `gdd step blueprint explore --zone <zone> --charter "<the charter>" --found <how many findings>`, and set the backlog item to done. Each finding goes to the backlog or to its fix: none stays only in the conversation.

Never on a calendar: an exploration comes when the blueprint's figure or the person calls for it, and its tokens and the environment it needs are its cost.
