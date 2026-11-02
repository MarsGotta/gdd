Feature: The report measures what the method promises
  `tminus report --promises` reads the log and the records and says, for every change and for each
  type and color of change, what the method promises to keep low: how many changes ended without
  stopping you, how many times each one stopped you (the sitting before its build, each decision
  at G4) and how many times it was parked, how long it waited for you (at the sitting and at G4),
  and the tokens and cost of its agents. A change ended without stopping you when it closed with
  no stop and never parked. Its color is the one its last build started with. `--since` keeps the
  changes created on or after a day, and `--until` reads the log as it was at the end of a day, or
  at a time, so two periods of the same project can be compared.

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
    And I run "tminus new-change a --title Drafts --type Feature --paths src/storage/db.rs"
    And I run "tminus new-change b --title Export --type Feature --paths src/storage/export.rs"
    And I run "tminus new-change c --title Typos --type Feature --paths docs/guide.md"
    And I run "tminus new-change d --title Crash --type Hotfix --paths src/app.rs"
    And I run "tminus new-change e --title Sync --type Feature --paths src/sync.rs"
    And the log says:
      """
      {"ts": "2026-10-20T09:00:00+00:00", "change": "a", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Drafts"}
      {"ts": "2026-10-20T09:30:00+00:00", "change": "a", "phase": "1-discover", "gate": "zones", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T09:30:00+00:00", "change": "a", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T09:30:00+00:00", "change": "a", "phase": "1-discover", "gate": "plan", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T09:40:00+00:00", "change": "a", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "red \u00b7 full"}
      {"ts": "2026-10-20T10:00:00+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 1000, "tokens_cache": 4000, "tokens_out": 800, "cost_usd": 1.2, "duration_s": 1200, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T10:10:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 500, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.1, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T10:20:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "demo", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T11:20:00+00:00", "change": "a", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "polish", "retry": 0, "note": "Shorter messages"}
      {"ts": "2026-10-20T11:25:00+00:00", "change": "a", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "red \u00b7 full"}
      {"ts": "2026-10-20T11:40:00+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 200, "tokens_cache": 1000, "tokens_out": 200, "cost_usd": 0.3, "duration_s": 600, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T11:45:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "demo", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T12:15:00+00:00", "change": "a", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": "ok"}
      {"ts": "2026-10-20T12:15:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T12:15:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T08:00:00+00:00", "change": "b", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Export"}
      {"ts": "2026-10-21T08:20:00+00:00", "change": "b", "phase": "1-discover", "gate": "zones", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T08:20:00+00:00", "change": "b", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T08:20:00+00:00", "change": "b", "phase": "1-discover", "gate": "plan", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T08:30:00+00:00", "change": "b", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "red \u00b7 full"}
      {"ts": "2026-10-21T08:50:00+00:00", "change": "b", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 400, "tokens_cache": 2000, "tokens_out": 400, "cost_usd": 0.5, "duration_s": 900, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T09:00:00+00:00", "change": "b", "phase": "-", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "parked", "retry": 0, "note": "G3 still red after one fix"}
      {"ts": "2026-10-21T09:40:00+00:00", "change": "b", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "red \u00b7 full"}
      {"ts": "2026-10-21T09:50:00+00:00", "change": "b", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 300, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.1, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T10:00:00+00:00", "change": "b", "phase": "4-release", "gate": "-", "role": "machine", "agent": "demo", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T10:30:00+00:00", "change": "b", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": "ok"}
      {"ts": "2026-10-21T10:30:00+00:00", "change": "b", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T10:30:00+00:00", "change": "b", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T09:00:00+00:00", "change": "c", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Typos"}
      {"ts": "2026-10-22T09:01:00+00:00", "change": "c", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "green \u00b7 fast"}
      {"ts": "2026-10-22T09:10:00+00:00", "change": "c", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 100, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.05, "duration_s": 500, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T09:20:00+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-22T09:20:00+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-22T12:00:00+00:00", "change": "d", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Crash"}
      {"ts": "2026-10-22T12:02:00+00:00", "change": "d", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 fast"}
      {"ts": "2026-10-22T12:10:00+00:00", "change": "d", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 50, "tokens_cache": 0, "tokens_out": 50, "cost_usd": 0.02, "duration_s": 400, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T12:15:00+00:00", "change": "d", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-x", "tokens_in": 50, "tokens_cache": 0, "tokens_out": 20, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": "", "cost_basis": "unknown"}
      {"ts": "2026-10-22T12:20:00+00:00", "change": "d", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-22T12:20:00+00:00", "change": "d", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "simulated", "retry": 0, "note": ""}
      {"ts": "2026-10-23T10:00:00+00:00", "change": "e", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Sync"}
      {"ts": "2026-10-23T10:45:00+00:00", "change": "e", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-23T10:50:00+00:00", "change": "e", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-23T11:00:00+00:00", "change": "e", "phase": "2-build", "gate": "G2", "role": "builder-test", "agent": "claude", "model": "sonnet", "tokens_in": 200, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.03, "duration_s": 300, "result": "ok", "retry": 0, "note": ""}
      """

  Scenario: Tasks that ended without stopping you
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Promises · 5 change(s) created from 2026-10-20 to 2026-10-23"
    And the output contains "All: 5 change(s) · 4 closed · 2 ended without stopping you"

  Scenario: Each change stops you at the sitting and at every decision at G4, and parks apart
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Stops: 1.2 per change (6 in 5) · parked 0.2 per change (1 in 5)"

  Scenario: How long each change waited for you
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Waiting for you: 43m per change (3h 35m in 5) · at the sitting 1h 35m · at G4 2h 00m"

  Scenario: Tokens and cost per change
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Tokens per change: in 560 · cache 1,400 · out 374"
    And the output contains "Cost: 0.4600 USD per change · in all 2.3000 USD + unknown (1 call(s) with no price)"

  Scenario: The same, for each type and color of change
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Feature · green: 1 change(s) · 1 closed · 1 ended without stopping you"
    And the output contains "Feature · red: 2 change(s) · 2 closed · 0 ended without stopping you"
    And the output contains "Stops: 2.5 per change (5 in 2) · parked 0.5 per change (1 in 2)"
    And the output contains "Waiting for you: 1h 25m per change (2h 50m in 2) · at the sitting 50m · at G4 2h 00m"
    And the output contains "Feature · yellow: 1 change(s) · 0 closed · 0 ended without stopping you"
    And the output contains "Hotfix · yellow: 1 change(s) · 1 closed · 1 ended without stopping you"

  Scenario: A period, from the day a change was created to the end of another day
    When I run "tminus report --promises --since 2026-10-21 --until 2026-10-22"
    Then it succeeds
    And the output contains "Promises · 3 change(s) created from 2026-10-21 to 2026-10-22"
    And the output contains "All: 3 change(s) · 3 closed · 2 ended without stopping you"
    And the output does not contain "Feature · yellow"

  Scenario: Until a time, the log as it was then
    When I run "tminus report --promises --until 2026-10-20T11:00:00+00:00"
    Then it succeeds
    And the output contains "All: 1 change(s) · 0 closed · 0 ended without stopping you"
    And the output contains "Stops: 1.0 per change (1 in 1) · parked 0.0 per change (0 in 1)"
    And the output contains "Waiting for you: 30m per change (30m in 1) · at the sitting 30m · at G4 0s"

  Scenario: A period without changes says so
    When I run "tminus report --promises --since 2026-10-24"
    Then it succeeds
    And the output contains "No change in the log yet."
