@hidden
Feature: gdd learn (hidden examples)

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

  Scenario: A tool that cannot run says so and the others still run
    Given the gate command "learn" "duplication" is "no-such-dup-tool-abc"
    When I run "gdd learn"
    Then it succeeds
    And the output contains "duplication: could not run (exit code 127)"
    And the output contains "unused: found something (exit code 3)"

  Scenario: Only the changes created since a day
    When I run "gdd learn --since 2026-10-22"
    Then it succeeds
    And the output contains "Polish notes at G4 (0)"
