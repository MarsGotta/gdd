# Discover · 2026-10-27 · Stack sheets

Participants: the human (stakeholder) and the Orchestrator. Builds on `2026-10-24-mvp.md`
(stack catalog: Rust, TypeScript, Python; tool catalog checked 2026-10-25).

## 1. One or several sheets per project (decided)

A project picks **one or more** stack sheets and gdd combines them: every gate runs the commands of
every chosen sheet (Rust's lint and TypeScript's lint, both test suites...), and the protected
paths add up. No sheet per combination.

Why: common desktop stacks mix languages. Tauri (112k★, Rust core and TypeScript UI) and Electron
(123k★, TypeScript, often with a Python or Rust helper), checked 2026-10-27 with `gh api`. The
pilot (Vivarium, a desktop app) has not picked its stack yet.

Alternative not chosen: one sheet per project plus hand-made combined sheets ("Tauri"): simpler at
first, but every new mix is one more sheet to maintain.

## 2. What the first change includes (decided)

Change `stack-sheets`: the three sheets as data inside gdd (written by the Orchestrator before the
sitting, like the agent sheets, with every tool command checked); `gdd stacks` lists them and shows
one; `gdd stacks compose <a> [<b>...]` shows the combination (gate commands and protected paths)
without writing anything; the per-stack instructions for the agents, installed by
`gdd agents install` when the project uses that sheet.

Left for `gdd init` (next change): writing the combination into `gdd/`, the Setup form and the
skeleton of a new project. Writing `gdd/` touches the harness, which belongs to the Setup, with the
human's form in front of them; this change only shows.

## 3. Per-stack instructions for the agents (decided)

One short instruction file per stack (about 40-60 lines), loaded on demand by the Builder and the
Reviewer only when they work in that stack: (1) how to write the scenario steps with its runner
(where they live, how each sentence binds to code; an undefined sentence is a failure, never
skipped); (2) where and how unit tests go, with the naming conventions; (3) what the gates usually
catch in that stack, from the Learn notes (e.g. every error branch needs its own test or mutation
testing leaves it alive); (4) never run the tools by hand: always through `bin/gdd g2`. Content
from each tool's official documentation and the Learn notes; Learn adds new stack rules there.

Alternative not chosen: no per-stack instructions, everything in each project's
`docs/standards.md`: simpler, but every project would rewrite the same.
