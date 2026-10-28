@hidden
Feature: Where the time goes, on the Board (hidden examples)

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
    And I run "gdd new-change d --title Search --paths src/search.rs"
    And the change "d" is in state "building"
    And the log says:
      """
      {"ts": "2026-10-28T08:00:00+00:00", "change": "d", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Search"}
      {"ts": "2026-10-28T08:30:00+00:00", "change": "d", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T08:40:00+00:00", "change": "d", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-28T09:10:00+00:00", "change": "d", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 10, "cost_usd": 0.0, "duration_s": 1800, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T09:11:00+00:00", "change": "d", "phase": "2-build", "gate": "G2", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T09:11:00+00:00", "change": "d", "phase": "-", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "parked", "retry": 0, "note": "G2 still red after 3 retries and 1 replan"}
      {"ts": "2026-10-28T10:11:00+00:00", "change": "d", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:20:00+00:00", "change": "d", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 540, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:20:00+00:00", "change": "d", "phase": "4-release", "gate": "-", "role": "machine", "agent": "demo", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:20:00+00:00", "change": "d", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ko", "retry": 0, "note": "Not what I asked"}
      """

  Scenario: The details of a change show where its time went
    When I open the board
    And I press Enter on the board
    Then the board shows "d · Search"
    And the board shows "Flow time 3h 20m · active 40m (20%) · waiting 2h 40m · between steps 0s"
    And the board shows "Waiting: parked 1h 00m (G2 still red after 3 retries and 1 replan)"

  Scenario: t shows the time of the whole project, the biggest waits first
    When I open the board
    And I press t on the board
    Then the board shows "Project: 1 change(s) · flow time 3h 20m"
    And the board shows "d · Search · rejected"
