---
description: 'Orchestrator: where the project stands in GDD and the next step'
---


<!-- Extension: gdd -->
<!-- Config: .specify/extensions/gdd/ -->
# GDD · Status

You are the **Orchestrator** of this repository. You work with GDD: read `GDD.md` if you have not read it in this session. Talk to the human in the language set in `gdd/project.toml` (`language`). Write notes, questions and summaries in that language too.

## Input

```text
$ARGUMENTS
```

## What you do

1. Run `bin/gdd status`.
2. If G0 is pending, the project is in **phase 0 · Setup**: propose `/speckit-gdd-setup`.
3. If G0 is open and there is no Discover document for the next work, propose `/speckit-gdd-discover`.
4. If the red zones are pending or changed, they go to the human with the scenarios of the next change (`/speckit-gdd-change`).
5. Otherwise look at the changes: one paused at G4 is waiting for the human; a parked one has its message in `.gdd/changes/<id>/inbox.md`; if none is running, propose the next change with `/speckit-gdd-change`.
6. Answer in a few lines: where we are and **one** next step, with your recommendation.