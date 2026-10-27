# Discover · 2026-10-27 · Parallel changes

Participants: the human (stakeholder) and the Orchestrator.

## Why now (decided)

The row was in `## 0.2`. Building gdd showed the bottleneck: a red change takes 30-60 minutes,
almost all of it agents working alone, and while it runs the main checkout belongs to it (the
recipe switches it to `change/<id>`), so not even the next sitting can be prepared there. The
human moved it into the MVP, right after security-exceptions, and wants to use it for the rest
of the MVP: same time per change, more changes per day.

## Shape (Orchestrator, from what gdd already has)

Two changes, both red (they are the recipe and the merge):

1. **parallel-launch**
   - `gdd launch <id>` runs the change in its own git worktree,
     `.gdd/worktrees/<id>` (ignored, like all of `.gdd/`), on `change/<id>`. The main checkout
     stays on the main branch, so the human and the Orchestrator keep working there.
   - One state for all: `.gdd/` of the main checkout (found through `git rev-parse
     --git-common-dir`), so `status`, the log, approvals and the Decision Inbox see every
     change wherever it runs. The binary that runs the gates stays `.gdd/bin/` of the main
     checkout, built on the main branch (a Builder can never run its own gdd).
   - Before launching, the planned paths of the change must not overlap those of any change in
     flight (building, fixing or waiting at G4); overlap is refused naming both changes and the
     paths. Same rule for two changes touching the supply chain or the harness.
   - A cap on changes building at once, `[parallel] max` in `gdd/rules.toml`, default 2: the
     agents' subscription limits are shared. A launch over the cap is refused, saying which
     changes are building.
2. **merge-queue**
   - Merges go one at a time (a lock in `.gdd/`). Before merging, the change branch takes the
     latest main and G3 runs again on top of it; red parks the change ("main moved under it"),
     green merges. Nothing reaches main that only worked with an older main.
   - The worktree is removed after the merge (or the KO); a parked change keeps it.

The human's stops do not change: one sitting and one G4 per change. Two G4s can arrive one
after the other, never merged at the same time.

## Order

security-exceptions (running) -> parallel-launch -> merge-queue -> the rest of the MVP in
parallel (init-adopt, init-form, Board, Sitting inbox, changelog, delivery-report,
spike-variants, closing the MVP).

## Sources

- git worktree: one repository, several working trees, each on its own branch; a branch can be
  checked out in one worktree only (git-worktree(1)).
- Merge queues (GitHub merge queue, bors, Mergify): each pull request is tested on top of the
  latest main plus the ones ahead of it before merging; the same idea, one at a time.

## Where the worktrees go (decided)

Researched (Orchestrator's report `research/2026-10-27-gdd-worktree-location.md`): tools put them
inside the repository (Claude Code `.claude/worktrees/`, with open reports of Jest and Vitest
collecting the copies and ESLint not reading `.gitignore` by default), in a folder of the tool
(Codex `$CODEX_HOME/worktrees`, Cursor `~/.cursor/worktrees`, Conductor
`~/conductor/workspaces/<repo>/`, Vibe Kanban `/var/tmp/vibe-kanban/worktrees`), or in a sibling
folder (Worktrunk's default `../<repo>.<branch>`, git's own examples, "the most popular
convention" per gitworktree.org). All make it configurable.

Decided: a sibling folder, `<repo>.worktrees/<id>` next to the repository, configurable in
`gdd/rules.toml`. The human chose it while saying it does not fully convince her: the default can
change later with one line of configuration.

## Split again (Orchestrator)

Every `.gdd/` path in gdd today hangs from the project root (about 240 places in 20 files). In a
worktree that root is the worktree, so the state would split per worktree. Three changes, not
two: **state-root** first (a Refactor: every `.gdd/` path goes through the main checkout, found
through git's common dir; nothing changes in a single checkout), then parallel-launch, then
merge-queue.
