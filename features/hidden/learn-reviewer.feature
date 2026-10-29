@hidden
Feature: Learn from the Reviewer (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "true"

      [learn]
      duplication = "echo Found 0 clones"
      unused = "echo src/old.py:3: unused function old_greeting; exit 3"
      """
    And everything is committed on main
    And the log says:
      """
      {"ts": "2026-10-20T09:00:00+00:00", "change": "a", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Drafts"}
      {"ts": "2026-10-20T11:00:00+00:00", "change": "a", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "polish", "retry": 0, "note": "Shorter error messages"}
      {"ts": "2026-10-21T09:00:00+00:00", "change": "b", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Export"}
      {"ts": "2026-10-21T11:00:00+00:00", "change": "b", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "polish", "retry": 0, "note": "Shorter error messages, again"}
      {"ts": "2026-10-21T12:00:00+00:00", "change": "b", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": "ok"}
      """

  Scenario: The same class twice in one change counts once for that change
    And the file ".gdd/changes/a/outputs/reviewer-1.md" says:
      """
      F1 · path followed through a link · src/a.rs:1 · x · none
      F2 · path followed through a link · src/b.rs:2 · y · none
      
      VERDICT: KO
      """
    And the file ".gdd/changes/b/outputs/reviewer-1.md" says:
      """
      F3 · path followed through a link · src/c.rs:3 · z · none
      
      VERDICT: KO
      """
    When I run "gdd learn"
    Then it succeeds
    And the output contains "path followed through a link · 2 changes (a, b)"
