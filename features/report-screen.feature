Feature: Where the time goes, on the Board
  The Board shows where each change's time went (the report of `gdd report --time`): in the
  details of a change, its flow time, active time, waiting and time between steps, by agent, gate
  command, release step and kind of waiting; with `t`, the whole project: what the changes add
  up to and the biggest waits first, then each change. The Board still writes nothing.

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
    And I run "gdd new-change a --title Drafts --paths src/storage/db.rs"
    And the change "a" is in state "closed"
    And I run "gdd new-change b --title Export --paths src/export.rs"
    And the change "b" is in state "building"
    And I run "gdd new-change c --title Notes --paths src/notes.rs"
    And the change "c" is in state "closed"
    And the log says:
      """
      {"ts": "2026-10-20T09:00:00+00:00", "change": "c", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Notes"}
      {"ts": "2026-10-20T09:10:00+00:00", "change": "c", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-20T09:20:00+00:00", "change": "c", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 200, "cost_usd": 0.0, "duration_s": 600, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-20T09:21:00+00:00", "change": "c", "phase": "2-build", "gate": "G2", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:00:00+00:00", "change": "a", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Drafts"}
      {"ts": "2026-10-28T10:20:00+00:00", "change": "a", "phase": "1-discover", "gate": "zones", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": "ok"}
      {"ts": "2026-10-28T10:21:00+00:00", "change": "a", "phase": "1-discover", "gate": "G1", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:21:00+00:00", "change": "a", "phase": "1-discover", "gate": "plan", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:30:00+00:00", "change": "a", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "red \u00b7 full"}
      {"ts": "2026-10-28T10:35:00+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "builder-test", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 100, "cost_usd": 0.0, "duration_s": 240, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:55:00+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 900, "cost_usd": 0.0, "duration_s": 1200, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:55:20+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "machine", "agent": "lint", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 20, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:56:00+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "machine", "agent": "unit", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 40, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T10:56:00+00:00", "change": "a", "phase": "2-build", "gate": "G2", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 60, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:00:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "acceptance_full", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 180, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:06:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "mutation", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 360, "result": "ko", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:06:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 540, "result": "ko", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:06:00+00:00", "change": "a", "phase": "-", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "parked", "retry": 0, "note": "G3 still red after one fix"}
      {"ts": "2026-10-28T11:46:00+00:00", "change": "a", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "red \u00b7 full \u00b7 G2 already green"}
      {"ts": "2026-10-28T11:50:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "acceptance_full", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 180, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:56:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "mutation", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 360, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:56:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 540, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:58:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 50, "cost_usd": 0.0, "duration_s": 120, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:58:00+00:00", "change": "a", "phase": "3-integrate", "gate": "G3", "role": "machine", "agent": "merge", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T12:00:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "demo", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 100, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T12:30:00+00:00", "change": "a", "phase": "4-release", "gate": "G4", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": "ok"}
      {"ts": "2026-10-28T12:30:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T12:30:00+00:00", "change": "a", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:00:00+00:00", "change": "b", "phase": "1-discover", "gate": "-", "role": "orchestrator", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "change-created", "retry": 0, "note": "Export"}
      {"ts": "2026-10-28T11:05:00+00:00", "change": "b", "phase": "1-discover", "gate": "zones", "role": "human", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:10:00+00:00", "change": "b", "phase": "2-build", "gate": "-", "role": "machine", "agent": "-", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0, "result": "prepared", "retry": 0, "note": "yellow \u00b7 full"}
      {"ts": "2026-10-28T11:30:00+00:00", "change": "b", "phase": "2-build", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 300, "cost_usd": 0.0, "duration_s": 1200, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:32:00+00:00", "change": "b", "phase": "2-build", "gate": "G2", "role": "machine", "agent": "unit", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 120, "result": "ok", "retry": 0, "note": ""}
      {"ts": "2026-10-28T11:32:00+00:00", "change": "b", "phase": "2-build", "gate": "G2", "role": "machine", "agent": "checks", "model": "-", "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 120, "result": "ok", "retry": 0, "note": ""}
      """

  Scenario: The details of a change show where its time went
    When I open the board
    And I press Enter on the board
    Then the board shows "b · Export"
    And the board shows "Flow time 32m · active 22m (69%) · waiting 10m · between steps 0s"
    And the board shows "Agents: builder-build · sonnet 20m"
    And the board shows "Waiting: at the sitting 5m, to start 5m"

  Scenario: A change that parked shows the cause of its wait
    When I open the board
    And I press d on the board
    And I press Down on the board
    And I press Enter on the board
    Then the board shows "a · Drafts"
    And the board shows "Gates: mutation 12m, acceptance_full 6m, unit 40s, lint 20s"
    And the board shows "Waiting: parked 40m (G3 still red after one fix)"

  Scenario: A change with nothing measured yet says so
    Given I run "gdd new-change z --title Guide --paths docs/guide.md"
    When I open the board
    And I press Enter on the board
    Then the board shows "z · Guide"
    And the board shows "Time: nothing measured yet"

  Scenario: t shows the time of the whole project, the biggest waits first
    When I open the board
    And I press t on the board
    Then the board shows "Where the time goes"
    And the board shows "Project: 3 change(s) · flow time 3h 23m"
    And the board shows "Biggest waits: parked 40m, for the human at G4 30m, at the sitting 26m"
    And the board shows "a · Drafts · closed"
    And the board shows "c · Notes · in progress"
    And the board does not show "Build (1)"

  Scenario: Esc goes back from the time to the list
    When I open the board
    And I press t on the board
    And I press Esc on the board
    Then the board shows "Build (1)"
    And the board does not show "Biggest waits"

  Scenario: The keys of the list name the time
    When I open the board
    Then the board shows "t time"

  Scenario: A log that cannot be read is shown, not hidden, and the list still works
    Given the log line 1 is replaced by "{not json"
    When I open the board
    Then the board shows "Build (1)"
    When I press t on the board
    Then the board shows "the log cannot be read"
