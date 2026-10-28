Feature: Delivery: DORA's five and GDD's signals
  `gdd report --dora` reads the log and the records and gives DORA's five software delivery
  metrics (dora.dev): change lead time, deployment frequency and failed deployment recovery time
  (throughput), change fail rate and deployment rework rate (instability); then GDD's own signals,
  signs to read and never targets: polish at G4, parks, Reviewer KOs, mutation testing red in G3,
  human stops and tokens per change. A deployment is a promotion (`promote` ok, or simulated in a
  project that simulates production); it failed when its observation failed or it was rolled
  back; a Hotfix deployment is unplanned rework.

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
    And I run "gdd new-change a --title Drafts --type Feature --paths src/storage/db.rs"
    And I run "gdd new-change b --title Crash --type Hotfix --paths src/app.rs"
    And I run "gdd new-change c --title Sync --type Feature --paths src/sync.rs"
    And I run "gdd new-change d --title Export --type Feature --paths src/export.rs"
    And I run "gdd new-change e --title Guide --type Feature --paths docs/guide.md"
    And the file ".gdd/changes/a/outputs/reviewer-1.md" says "VERDICT: KO"
    And the file ".gdd/changes/a/outputs/reviewer-2.md" says "VERDICT: OK"
    And the file ".gdd/changes/b/outputs/reviewer-1.md" says "VERDICT: OK"
    And the file ".gdd/changes/c/outputs/reviewer-1.md" says "VERDICT: OK"
    And the file ".gdd/changes/d/outputs/reviewer-1.md" says "VERDICT: OK"
    And the log says:
      """
      {"ts": "2026-10-20T09:00:00+00:00", "change": "a", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Drafts"}
      {"ts": "2026-10-20T09:20:00+00:00", "change": "a", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T09:30:00+00:00", "change": "a", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "red \u00b7 full"}
      {"ts": "2026-10-20T09:50:00+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 800, "cost_usd": 0.0, "duration_s": 1200, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T09:55:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "mutation", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 200, "result": "ko", "retry": 0, "note": ""}
      {"ts": "2026-10-20T10:00:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T10:15:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "mutation", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 200, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T10:20:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T10:40:00+00:00", "change": "a", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "polish", "retry": 0, "note": "Shorter messages"}
      {"ts": "2026-10-20T11:00:00+00:00", "change": "a", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": "ok"}
      {"ts": "2026-10-20T11:00:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T11:00:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T08:00:00+00:00", "change": "b", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Crash"}
      {"ts": "2026-10-21T08:05:00+00:00", "change": "b", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T08:10:00+00:00", "change": "b", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-21T08:30:00+00:00", "change": "b", "phase": "-", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "parked", "retry": 0, "note": "G3 still red after one fix"}
      {"ts": "2026-10-21T08:50:00+00:00", "change": "b", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-21T09:00:00+00:00", "change": "b", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 200, "cost_usd": 0.0, "duration_s": 300, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T09:05:00+00:00", "change": "b", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "mutation", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 100, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T09:08:00+00:00", "change": "b", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T09:10:00+00:00", "change": "b", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T09:10:00+00:00", "change": "b", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T12:00:00+00:00", "change": "c", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Sync"}
      {"ts": "2026-10-21T12:10:00+00:00", "change": "c", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T12:30:00+00:00", "change": "c", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-21T12:50:00+00:00", "change": "c", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 400, "cost_usd": 0.0, "duration_s": 600, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T13:00:00+00:00", "change": "c", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T13:30:00+00:00", "change": "c", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": "ok"}
      {"ts": "2026-10-21T13:30:00+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-21T13:35:00+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ko", "retry": 0, "note": ""}
      {"ts": "2026-10-21T13:40:00+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "rollback", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T09:00:00+00:00", "change": "d", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Export"}
      {"ts": "2026-10-22T09:10:00+00:00", "change": "d", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-22T09:30:00+00:00", "change": "d", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.0, "duration_s": 600, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T09:40:00+00:00", "change": "d", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T10:00:00+00:00", "change": "d", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T10:00:00+00:00", "change": "d", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-22T11:00:00+00:00", "change": "e", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Guide"}
      """

  Scenario: DORA's throughput
    When I run "gdd report --dora"
    Then it succeeds
    And the output contains "Delivery · 4 deployment(s) from 2026-10-20 to 2026-10-22"
    And the output contains "Change lead time: median 1h 00m, from the build's start to the promotion"
    And the output contains "Deployment frequency: 1.3 per day · median 20h 30m between deployments"
    And the output contains "Failed deployment recovery time: median 10m (1 failed)"

  Scenario: DORA's instability
    When I run "gdd report --dora"
    Then it succeeds
    And the output contains "Change fail rate: 25% (1 of 4)"
    And the output contains "Deployment rework rate: 25% (1 of 4 are Hotfixes)"

  Scenario: GDD's signals, never targets
    When I run "gdd report --dora"
    Then it succeeds
    And the output contains "GDD signals (signs to read, never targets)"
    And the output contains "Polish at G4: 33% (1 of 3 decisions)"
    And the output contains "Parks: 0.2 per change (1 in 5)"
    And the output contains "Reviewer KO: 20% (1 of 5 reviews)"
    And the output contains "Mutation testing red in G3: 1 of 3 runs"
    And the output contains "Human stops: 1.2 per change (6 in 5)"
    And the output contains "Tokens out: median 400 per change"

  Scenario: Only the changes created since a day
    When I run "gdd report --dora --since 2026-10-21"
    Then it succeeds
    And the output contains "Delivery · 3 deployment(s) from 2026-10-21 to 2026-10-22"
    And the output contains "Change fail rate: 33% (1 of 3)"
    And the output contains "Polish at G4: 0% (0 of 1 decisions)"

  Scenario: Without a deployment the signals still show
    When I run "gdd report --dora --since 2026-10-22"
    Then it succeeds
    And the output contains "Delivery · 1 deployment(s) from 2026-10-22 to 2026-10-22"
    When I run "gdd report --dora --since 2026-10-23"
    Then it succeeds
    And the output contains "No deployment in the log yet"
    And the output contains "GDD signals"

  Scenario: Machine-readable delivery
    When I run "gdd report --dora --json"
    Then it succeeds
    And the output contains "lead_time_s"
    And the output contains "change_fail_rate"
