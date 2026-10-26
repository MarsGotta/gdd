Feature: The last commands of the bootstrap kit, in gdd
  gdd does what the Python kit still did for this repository: the harness self-test that G0
  needs, what CI runs on a change branch, and adding the conversation with the human to the log.
  The commands the agents run without a change id find their change in GDD_CHANGE, as with the
  kit. After this change the kit can be deleted here.

  The self-test checks four things: a failing test turns G2 red, the harness files are protected
  and CI runs the gates, rollback works in pre (or is simulated), and zero scenarios counts as a
  failure. CI checks a change branch against the main branch: no harness path touched, G2 and G3
  green; it needs no local state, because .gdd/ never travels with the repository.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**", ".github/**"]
      builder_build = ["features/**"]
      builder_test = ["src/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "test ! -f tests/failing.txt"

      [g3]
      acceptance_full = "true"

      [selftest]
      file = "tests/failing.txt"
      content = "fails"

      [release]
      simulated = true
      rollback_pre = ""
      """
    And the file ".github/workflows/gdd.yml" says "run: bin/gdd ci"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main

  Scenario: A sound harness passes its self-test
    When I run "gdd selftest"
    Then it succeeds
    And the output contains "harness 4/4"
    And the file ".gdd/selftest.json" contains "harness 4/4"
    And the file "tests/failing.txt" does not exist

  Scenario: A G2 that misses a failing test fails the self-test
    Given the gate command "g2" "unit" is "true"
    When I run "gdd selftest"
    Then the exit code is not 0
    And the output contains "KO a failing test turns G2 red"
    And the output contains "harness 3/4"

  Scenario: The self-test gives back a file that was already there
    Given the file "tests/failing.txt" says "mine"
    When I run "gdd selftest"
    Then the file "tests/failing.txt" contains "mine"

  Scenario: A CI that does not run the gates fails the self-test
    Given the file ".github/workflows/gdd.yml" says "run: cargo test"
    When I run "gdd selftest"
    Then the exit code is not 0
    And the output contains "KO editing a protected file is blocked (Path Guard and CI)"

  Scenario: A rollback in pre that fails counts against the harness
    Given the gate command "release" "rollback_pre" is "exit 1"
    When I run "gdd selftest"
    Then the exit code is not 0
    And the output contains "KO rollback works in pre"

  Scenario: The status shows the self-test
    Given I run "gdd selftest"
    When I run "gdd status"
    Then the output contains "Harness self-test: harness 4/4"

  Scenario: On the main branch CI has nothing to check per change
    When I run "gdd ci"
    Then it succeeds
    And the output contains "nothing to check per change"

  Scenario: A change that touches the harness fails CI
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    When the change commits "bin/tool"
    And I run "gdd ci"
    Then the exit code is not 0
    And the output contains "bin/tool: harness path changed inside a change"

  Scenario: A change whose G2 is red fails CI
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    When the change commits "tests/failing.txt"
    And I run "gdd ci"
    Then the exit code is not 0
    And the output contains "G2 red: unit"

  Scenario: A green change passes CI without any local state
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    And the local state is gone
    When the change commits "src/app.txt"
    And I run "gdd ci"
    Then it succeeds
    And the output contains "G2 green"
    And the output contains "G3 green"

  Scenario: CI reads the branch of a pull request from GitHub
    Given the environment has "GITHUB_HEAD_REF" set to "change/c1"
    When I run "gdd ci"
    Then it succeeds
    And the output does not contain "nothing to check per change"
    And the output contains "G3 green"

  Scenario: The conversation with the human goes to the log
    Given the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    And the Claude Code session "s1" used "claude-opus-5-5" with 2 output tokens
    When I run "gdd session --phase 1-discover"
    Then it succeeds
    And the output contains "Session imported: 2 new requests across 1 model(s), phase 1-discover."
    When I run "gdd report --by agent --json"
    Then the entry with "agent" "claude-interactive" has "tokens_out" 42 and "calls" 1

  Scenario: The same requests are never counted twice
    Given the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    And I run "gdd session --phase 1-discover"
    When I run "gdd session --phase 1-discover"
    Then it succeeds
    And the output contains "Session imported: 0 new requests across 0 model(s), phase 1-discover."

  Scenario: Sessions started by the recipe are left out
    Given the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    And the file ".gdd/headless_sessions.txt" says "s1"
    When I run "gdd session --phase 2-build"
    Then the output contains "Session imported: 0 new requests across 0 model(s), phase 2-build."

  Scenario: A session can be counted for one change
    Given the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    When I run "gdd session --phase 1-discover --change c1"
    And I run "gdd report --by change --json"
    Then the entry with "change" "c1" has "tokens_out" 40 and "calls" 1

  Scenario: Commands the agents run find their change in GDD_CHANGE
    Given I run "gdd new-change c1 --title Docs --paths docs/a.md"
    And the environment has "GDD_CHANGE" set to "c1"
    When I run "gdd classify"
    Then it succeeds
    And the output contains "green"

  Scenario: Without an id or GDD_CHANGE the command says what is missing
    When I run "gdd g2"
    Then it fails with a message that contains "missing change id (argument or GDD_CHANGE)"
