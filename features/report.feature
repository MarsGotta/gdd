Feature: Report token usage
  Every agent call leaves a line in .gdd/log.jsonl with its tokens, cost and time.
  The report adds them up by role, model, phase or change, so the human can see
  where the tokens go. Machine lines without tokens are not agent calls.

  Background:
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

  Scenario: Report by role
    When I run "gdd report --by role"
    Then the output is:
      """
      role -> calls · tokens in · cache · out · cost USD · seconds
      builder-build -> 3 · 350 · 1,000 · 55 · 0.0310 · 14
      reviewer -> 1 · 1,000 · 0 · 40 · 0.0000 · 9
      TOTAL -> in 1,350 · cache 1,000 · out 95 · 0.0310 USD
      Human stops logged: 2
      """

  Scenario: Report for one change
    When I run "gdd report --by role --change a"
    Then the output contains "builder-build -> 2 · 300 · 1,000 · 50 · 0.0300 · 12"
    And the output contains "TOTAL -> in 1,300 · cache 1,000 · out 90 · 0.0300 USD"

  Scenario: Report by two keys
    When I run "gdd report --by model,role"
    Then the output contains "model · role -> calls · tokens in · cache · out · cost USD · seconds"
    And the output contains "opus · builder-build -> 2 · 300 · 1,000 · 50 · 0.0300 · 12"
    And the output contains "sonnet · builder-build -> 1 · 50 · 0 · 5 · 0.0010 · 2"

  Scenario: The default grouping is by role
    When I run "gdd report"
    Then the output contains "builder-build -> 3 · 350 · 1,000 · 55 · 0.0310 · 14"

  Scenario: A change with no agent calls
    When I run "gdd report --change zzz"
    Then the output is:
      """
      No agent calls in the log yet.
      """

  Scenario: Machine-readable report
    When I run "gdd report --by role --json"
    Then the output is a JSON list of 2 entries
    And the entry with "role" "reviewer" has "tokens_in" 1000 and "calls" 1

  Scenario: A broken line in the log is left out, and the report says so
    Given the log line 3 is replaced by "{not json"
    When I run "gdd report"
    Then it succeeds
    And the output contains "1 line of the log is not one of gdd's: left out"
