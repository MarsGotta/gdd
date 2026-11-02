@hidden
Feature: The report measures what the method promises (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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
    And I run "tminus new-change f --title Rename --type Refactor --paths src/names.rs"
    And I run "tminus new-change g --title Tidy --type Refactor --paths src/tidy.rs"
    And I run "tminus new-change h --title Move --type Migration --paths docs/move.md"
    And the log says:
      """
      {"ts": "2026-10-25T09:00:00+00:00", "change": "f", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Rename"}
      {"ts": "2026-10-25T09:10:00+00:00", "change": "f", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-25T09:15:00+00:00", "change": "f", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-25T09:30:00+00:00", "change": "f", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 300, "tokens_cache": 900, "tokens_out": 60, "cost_usd": 0.0021, "duration_s": 600, "result": "ok", "retry": 0, "note": "", "cost_basis": "estimated"}
      {"ts": "2026-10-25T09:40:00+00:00", "change": "f", "phase": "4-release", "gate": "-", "role": "machine", "agent": "demo", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-25T10:40:00+00:00", "change": "f", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ko", "retry": 0, "note": "Not this way"}
      {"ts": "2026-10-25T11:00:00+00:00", "change": "g", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Tidy"}
      {"ts": "2026-10-25T11:02:00+00:00", "change": "g", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 fast"}
      {"ts": "2026-10-25T11:20:00+00:00", "change": "g", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 100, "tokens_cache": 300, "tokens_out": 40, "cost_usd": 0.0045, "duration_s": 600, "result": "ok", "retry": 0, "note": "", "cost_basis": "estimated"}
      {"ts": "2026-10-25T11:30:00+00:00", "change": "g", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-25T11:30:00+00:00", "change": "g", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-26T08:00:00+00:00", "change": "h", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Move"}
      {"ts": "2026-10-26T08:01:00+00:00", "change": "h", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "green \u00b7 fast"}
      {"ts": "2026-10-26T08:20:00+00:00", "change": "h", "phase": "-", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "parked", "retry": 0, "note": "G2 still red after 3 retries and 1 replan"}
      {"ts": "2026-10-26T09:20:00+00:00", "change": "h", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "green \u00b7 fast"}
      {"ts": "2026-10-26T09:30:00+00:00", "change": "h", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 200, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.01, "duration_s": 300, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-26T09:40:00+00:00", "change": "h", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-26T09:40:00+00:00", "change": "h", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      """

  Scenario: Tasks that ended without stopping you, a parked one and a rejected one
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "All: 3 change(s) · 2 closed · 1 ended without stopping you"
    And the output contains "Migration · green: 1 change(s) · 1 closed · 0 ended without stopping you"
    And the output contains "Refactor · yellow: 2 change(s) · 1 closed · 1 ended without stopping you"
    And the output contains "Stops: 1.0 per change (2 in 2) · parked 0.0 per change (0 in 2)"
    And the output contains "Waiting for you: 35m per change (1h 10m in 2) · at the sitting 10m · at G4 1h 00m"
    And the output contains "Cost: ~0.0055 USD per change · in all ~0.0166 USD"

  Scenario: Until a time, the log as it was then
    When I run "tminus report --promises --until 2026-10-25T10:00:00+00:00"
    Then it succeeds
    And the output contains "Promises · 1 change(s) created from 2026-10-25 to 2026-10-25"
    And the output contains "All: 1 change(s) · 0 closed · 0 ended without stopping you"
    And the output contains "Waiting for you: 10m per change (10m in 1) · at the sitting 10m · at G4 0s"
