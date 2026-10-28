Feature: Where the time goes
  `gdd report --time` reads the log and splits each change's flow time, from its creation to its
  last line, into active time (the agents' calls by role and model, the gates by command, the
  release steps) and waiting (at the sitting, until it starts, parked until relaunched, for the
  human at G4); what is left is the time between steps. Flow efficiency is active time over
  flow time (Flow Framework). The project line adds every change up and names the biggest
  waits, so the human can see whether GDD saves time and which step to polish.

  Background:
    Given a GDD project with the log:
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

  Scenario: Where the time of one change goes
    When I run "gdd report --time --change a"
    Then it succeeds
    And the output contains "a · Drafts · closed"
    And the output contains "Flow time 2h 30m · active 47m (31%) · waiting 1h 40m · between steps 3m"
    And the output contains "Agents: builder-build · opus 20m, builder-test · sonnet 4m, reviewer · gpt-6 2m"
    And the output contains "Gates: mutation 12m, acceptance_full 6m, unit 40s, lint 20s"
    And the output contains "Release: demo 2m"
    And the output contains "Waiting: parked 40m (G3 still red after one fix), for the human at G4 30m, at the sitting 21m, to start 9m"

  Scenario: A change still in progress counts until its last line
    When I run "gdd report --time --change b"
    Then it succeeds
    And the output contains "b · Export · in progress"
    And the output contains "Flow time 32m · active 22m (69%) · waiting 10m · between steps 0s"
    And the output contains "Waiting: at the sitting 5m, to start 5m"

  Scenario: A change created without a sitting waits only to start
    When I run "gdd report --time --change c"
    Then it succeeds
    And the output contains "Flow time 21m · active 11m (52%) · waiting 10m · between steps 0s"
    And the output contains "Waiting: to start 10m"

  Scenario: The project adds every change up and names the biggest waits
    When I run "gdd report --time"
    Then it succeeds
    And the output contains "Project: 3 change(s) · flow time 3h 23m · active 1h 20m (39%) · waiting 2h 00m · between steps 3m"
    And the output contains "Biggest waits: parked 40m, for the human at G4 30m, at the sitting 26m"

  Scenario: Only the changes created since a day
    When I run "gdd report --time --since 2026-10-28"
    Then it succeeds
    And the output contains "Project: 2 change(s)"
    And the output does not contain "c · Notes"

  Scenario: Machine-readable time
    When I run "gdd report --time --change a --json"
    Then it succeeds
    And the output is a JSON list of 1 entries
    And the entry with "change" "a" has "flow_s" 9000 and "active_s" 2800

  Scenario: A change not in the log
    When I run "gdd report --time --change zzz"
    Then it succeeds
    And the output contains "No change zzz in the log."
