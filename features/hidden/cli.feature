@hidden
Feature: Hidden examples for the CLI

  Scenario: Report by phase
    Given a GDD project with the log:
      """
      {"change": "a", "phase": "2-build", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 100, "tokens_cache": 400, "tokens_out": 20, "cost_usd": 0.01, "duration_s": 5}
      {"change": "a", "phase": "2-build", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 200, "tokens_cache": 600, "tokens_out": 30, "cost_usd": 0.02, "duration_s": 7}
      {"change": "a", "phase": "3-integrate", "role": "reviewer", "agent": "codex", "model": "gpt-x", "tokens_in": 1000, "tokens_cache": 0, "tokens_out": 40, "cost_usd": 0.0, "duration_s": 9}
      {"change": "b", "phase": "2-build", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 50, "tokens_cache": 0, "tokens_out": 5, "cost_usd": 0.001, "duration_s": 2}
      {"change": "a", "phase": "2-build", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 3, "result": "ko"}
      {"change": "a", "gate": "G1", "role": "human", "result": "ok"}
      {"change": "a", "gate": "G4", "role": "human", "result": "ok"}
      """
    When I run "gdd report --by phase"
    Then the output contains "2-build -> 3 · 350 · 1,000 · 55 · 0.0310 · 14"
    And the output contains "3-integrate -> 1 · 1,000 · 0 · 40 · 0.0000 · 9"

  Scenario: An empty log
    Given a GDD project with the log:
      """
      """
    When I run "gdd report"
    Then the output is:
      """
      No agent calls in the log yet.
      """

  Scenario: A change that cannot be resumed takes the full lane
    Given a GDD project with the zone map:
      """
      default = "yellow"
      """
    And the project's rules table is the default one
    And the planned change "x" touches "src/a.rs" and is not resumable
    When I run "gdd classify x"
    Then the JSON field "color" is "yellow"
    And the JSON field "lane" is "full"
