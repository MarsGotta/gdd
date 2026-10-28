@hidden
Feature: Delivery: DORA's five and GDD's signals (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And everything is committed on main
    And I run "gdd new-change f --title Tags --type Feature --paths src/tags.rs"
    And I run "gdd new-change g --title Crash --type Hotfix --paths src/app.rs"
    And the log says:
      """
      {"ts": "2026-10-25T09:00:00+00:00", "change": "f", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Tags"}
      {"ts": "2026-10-25T09:10:00+00:00", "change": "f", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-25T09:40:00+00:00", "change": "f", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 50, "cost_usd": 0.0, "duration_s": 600, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-25T10:10:00+00:00", "change": "f", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-25T10:10:00+00:00", "change": "f", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-26T09:00:00+00:00", "change": "g", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Crash"}
      {"ts": "2026-10-26T09:30:00+00:00", "change": "g", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-26T10:00:00+00:00", "change": "g", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-26T10:00:00+00:00", "change": "g", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      """

  Scenario: DORA's throughput
    When I run "gdd report --dora"
    Then it succeeds
    And the output contains "Delivery · 2 deployment(s) from 2026-10-25 to 2026-10-26 · 2 simulated"
    And the output contains "Change lead time: median 45m, from the build's start to the promotion"
    And the output contains "Deployment frequency: 1.0 per day · median 23h 50m between deployments"

  Scenario: DORA's instability
    When I run "gdd report --dora"
    Then it succeeds
    And the output contains "Change fail rate: 0% (0 of 2)"
    And the output contains "Deployment rework rate: 50% (1 of 2 are Hotfixes)"
