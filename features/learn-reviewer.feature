Feature: Learn from the Reviewer
  The Reviewer names each finding with its class of bug (F1 · <class> · file:line ...). When the
  same class shows up in several changes, it is a lesson: a line for docs/standards.md, a check
  or a test the Builder writes first. `gdd learn` lists, next to the polish notes at G4, the
  classes of findings that repeat across changes, and how many reviews named no class at all, so
  a Reviewer that does not follow the format is seen.

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

  Scenario: A class of finding that repeats across changes is listed
    And the file ".gdd/changes/a/outputs/reviewer-1.md" says:
      """
      F1 · unchecked input · src/import.rs:12 · a size from the file is used as is · src/export.rs:40
      F2 · error branch without a test · src/import.rs:30 · the error path is never run · none
      
      VERDICT: KO
      """
    And the file ".gdd/changes/b/outputs/reviewer-1.md" says:
      """
      F1 · Unchecked input · src/export.rs:8 · the name is not checked · none
      
      VERDICT: KO
      """
    When I run "gdd learn"
    Then it succeeds
    And the output contains "Reviewer findings that repeat (1)"
    And the output contains "unchecked input · 2 changes (a, b)"

  Scenario: A class found in one change only is not listed
    And the file ".gdd/changes/a/outputs/reviewer-1.md" says:
      """
      F1 · unchecked input · src/import.rs:12 · a size from the file is used as is · src/export.rs:40
      F2 · error branch without a test · src/import.rs:30 · the error path is never run · none
      
      VERDICT: KO
      """
    And the file ".gdd/changes/b/outputs/reviewer-1.md" says:
      """
      F1 · Unchecked input · src/export.rs:8 · the name is not checked · none
      
      VERDICT: KO
      """
    When I run "gdd learn"
    Then it succeeds
    And the output does not contain "error branch without a test"

  Scenario: A review that ends OK without findings counts as a review with nothing to name
    And the file ".gdd/changes/a/outputs/reviewer-1.md" says:
      """
      F1 · unchecked input · src/import.rs:12 · a size from the file is used as is · src/export.rs:40
      F2 · error branch without a test · src/import.rs:30 · the error path is never run · none
      
      VERDICT: KO
      """
    And the file ".gdd/changes/b/outputs/reviewer-1.md" says:
      """
      F1 · Unchecked input · src/export.rs:8 · the name is not checked · none
      
      VERDICT: KO
      """
    And the file ".gdd/changes/b/outputs/reviewer-2.md" says:
      """
      F1 fixed.
      
      VERDICT: OK
      """
    When I run "gdd learn"
    Then it succeeds
    And the output contains "Reviews whose findings name no class: none"

  Scenario: Reviews whose findings name no class are counted
    And the file ".gdd/changes/a/outputs/reviewer-1.md" says:
      """
      F1 · unchecked input · src/import.rs:12 · a size from the file is used as is · src/export.rs:40
      F2 · error branch without a test · src/import.rs:30 · the error path is never run · none
      
      VERDICT: KO
      """
    And the file ".gdd/changes/b/outputs/reviewer-1.md" says:
      """
      - [P2] src/export.rs:8: the name is not checked.
      
      VERDICT: KO
      """
    When I run "gdd learn"
    Then it succeeds
    And the output contains "Reviews whose findings name no class: 1 of 2 (b)"

  Scenario: Only the changes created since a day
    And the file ".gdd/changes/a/outputs/reviewer-1.md" says:
      """
      F1 · unchecked input · src/import.rs:12 · a size from the file is used as is · src/export.rs:40
      F2 · error branch without a test · src/import.rs:30 · the error path is never run · none
      
      VERDICT: KO
      """
    And the file ".gdd/changes/b/outputs/reviewer-1.md" says:
      """
      F1 · Unchecked input · src/export.rs:8 · the name is not checked · none
      
      VERDICT: KO
      """
    When I run "gdd learn --since 2026-10-21"
    Then it succeeds
    And the output contains "Reviewer findings that repeat: none"

  Scenario: Without reviews there is nothing to learn from them
    When I run "gdd learn"
    Then it succeeds
    And the output contains "Reviewer findings that repeat: none"
