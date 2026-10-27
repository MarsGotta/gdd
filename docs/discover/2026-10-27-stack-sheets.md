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
