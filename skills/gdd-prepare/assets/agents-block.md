<!-- gdd:begin -->
## GDD

This project uses Gate-Driven Development (contract gdd/1): a change is done when its gates pass, not when an agent says so.

- No role given? Load the `gdd` skill first.
- Edit only what your role owns: never a sealed check, and never `gdd/`, `features/` or the agents' own files (`AGENTS.md`, skills, settings) unless your role writes them.
- Get through gates with facts: the command, its exit code and its last lines.
- When something doesn't fit, stop and say why.

Map: `gdd/blueprint.md` zones and gates, `gdd/gates/` their scenarios, `gdd/briefs/` changes, `features/` their scenarios, `gdd/context/` references.

@gdd/constitution.md
<!-- gdd:end -->
