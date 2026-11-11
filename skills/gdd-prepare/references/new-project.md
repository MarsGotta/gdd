# Preparing a new project

There's no code to learn from, so everything the agents need goes in the constitution: the stack, the tools and every command.

- **The stack.** If the person has one in mind, take it. If not, propose what is usual for that kind of product, with the reason in one line, and let the person choose. It comes before the gates, because their checks are written for it: in one run, with no stack chosen, the gate for new dependencies had to guess every format a manifest could take, and each review found one more.
- **The zones.** Propose them from what the product will do, before any code exists: money, accounts, personal data, what talks to outside services, deployment. Ask only what would change a zone or a gate.
- **The commands from day one.** Build, test and lint commands exist before the first function does, and CI runs them, so the checks run out of the agents' reach from the first change. Setting CI up is the first item of the backlog.
- **The backlog.** After CI, its first items build each zone's gates and try to fool them. Then advise where to start: functions with little risk first, so the gates get tried where a mistake is cheap, and money only once its gates have held.
