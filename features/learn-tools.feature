Feature: gdd learn
  Learn has no gate: it only hardens. `gdd learn` runs the Learn tools of `gdd/commands.toml`
  (`[learn]`: duplication, unused code; warnings, never a gate) on the project, keeps what each one
  said in `.gdd/learn/`, and lists the polish notes the human gave at G4, so the Orchestrator can
  spot the ones that repeat and turn them into a standard or a check. It never fails because a
  tool found something.

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

  Scenario: Each Learn tool runs, and what it found is a warning
    When I run "gdd learn"
    Then it succeeds
    And the output contains "duplication: nothing found"
    And the output contains "unused: found something (exit code 3)"
    And the output contains "src/old.py:3: unused function old_greeting"

  Scenario: What each tool said is kept
    When I run "gdd learn"
    Then it succeeds
    And the output contains "All it said: .gdd/learn/unused.txt"
    And the file ".gdd/learn/unused.txt" contains "unused function old_greeting"

  Scenario: A tool that cannot run says so and the others still run
    Given the gate command "learn" "unused" is "no-such-learn-tool-xyz"
    When I run "gdd learn"
    Then it succeeds
    And the output contains "unused: could not run (exit code 127)"
    And the output contains "duplication: nothing found"

  Scenario: Without Learn tools it says how to add them
    Given the gate commands:
      """
      [g2]
      unit = "true"
      """
    When I run "gdd learn"
    Then it succeeds
    And the output contains "No Learn tools in gdd/commands.toml"
    And the output contains "gdd stacks"

  Scenario: The polish notes at G4, to spot the ones that repeat
    When I run "gdd learn"
    Then it succeeds
    And the output contains "Polish notes at G4 (2)"
    And the output contains "a: Shorter error messages"
    And the output contains "b: Shorter error messages, again"

  Scenario: Only the changes created since a day
    When I run "gdd learn --since 2026-10-21"
    Then it succeeds
    And the output contains "Polish notes at G4 (1)"
    And the output does not contain "a: Shorter error messages"

  Scenario: A Builder cannot run Learn
    When the role "builder-build" runs "gdd learn"
    Then it fails with a message that contains "Builders and Reviewers cannot"
