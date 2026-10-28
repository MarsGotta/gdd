@hidden
Feature: Where the time goes (hidden examples)

  Background:
    Given a GDD project with the log:
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

  Scenario: Where the time of one change goes
    When I run "gdd report --time --change d"
    Then it succeeds
    And the output contains "d · Search · rejected"
    And the output contains "Flow time 3h 20m · active 40m (20%) · waiting 2h 40m · between steps 0s"
    And the output contains "Waiting: parked 1h 00m (G2 still red after 3 retries and 1 replan), for the human at G4 1h 00m, at the sitting 30m, to start 10m"

  Scenario: The project adds every change up and names the biggest waits
    When I run "gdd report --time"
    Then it succeeds
    And the output contains "Project: 1 change(s) · flow time 3h 20m · active 40m (20%) · waiting 2h 40m · between steps 0s"
