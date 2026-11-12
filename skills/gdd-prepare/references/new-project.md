# Preparing a new project

There's no code to learn from, so everything the agents need goes in the constitution: the stack, the tools and every command.

- **The stack.** If the request or the person names one, that is the stack, and the constitution uses it as named: in one run the request named a language and the constitution chose another. If not, propose what is usual for that kind of product, with the reason in one line, and let the person choose. It comes before the checks, because they are set up for it: in one run, with no stack chosen, a check on new dependencies had to guess every format a manifest could take, and each review found one more.
- **The zones.** Propose them from what the product will do, before any code exists: money, accounts, personal data, what talks to outside services, deployment. Ask only what would change a zone or a gate.
- **The commands from day one.** The commands to build, test, lint and scan for secrets exist before the first function does, and CI runs them, so the checks run out of the agents' reach from the first change. Setting them up, with CI, is the first item of the backlog.
- **The backlog.** Its first item sets up the standing checks and CI, and they get tried once. Then advise where to start: functions with little risk first, so the checks get tried where a mistake is cheap, and money only once they have held.
