Feature: The sitting at G4
  When a change waits for the human's G4, `gdd sitting <change>` shows what to judge it by: the
  demo, the Reviewer's verdict and words, the decisions taken without the human, its changelog
  line and where its time went. `o` approves, `k` rejects and `p` asks for a polish round, KO and
  polish with the human's words, always through a confirmation that shows the exact command and
  what follows. After `y` the screen closes and gdd runs that `gdd resume` in the terminal, so its
  progress shows; nothing runs before `y`.

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
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And the models:
      """
      [orchestrator]
      integration = "claude"
      model = "opus"

      [builder.yellow]
      integration = "claude"
      model = "sonnet"

      [builder.red]
      integration = "claude"
      model = "opus"

      [builder_test]
      integration = "claude"
      model = "sonnet"

      [reviewer]
      integration = "codex"
      model = "gpt-6"

      [families]
      "opus" = "anthropic"
      "sonnet" = "anthropic"
      "gpt" = "openai"
      """
    And the gate commands:
      """
      [g2]
      unit = "test -f src/app.txt"

      [g3]
      acceptance_full = "true"

      [release]
      simulated = true
      build = "true"
      deploy_pre = "true"
      tests_pre = "true"
      demo = ""
      promote = ""
      observe = ""
      rollback = ""
      """
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    And the Setup is done
    And the red zones are approved
    And the agents are fakes

  Scenario: The G4 page shows what the change waits for
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    Then the sitting shows "c1 · App · waiting for your G4"
    And the sitting shows "Reviewer: OK"
    And the sitting shows "Demo: none, the project has no demo command"
    And the sitting shows "Flow time"
    And the sitting shows "o OK · k KO · p polish"

  Scenario: The demo shows its numbers
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the file ".gdd/changes/c1/demo/scenarios.txt" says "12 scenarios (12 passed)"
    When I open the sitting of "c1"
    Then the sitting shows "Demo: 12 scenarios (12 passed) · .gdd/changes/c1/demo/scenarios.txt"

  Scenario: The decisions taken without the human show
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And I run "gdd note c1 decided Took the short path"
    When I open the sitting of "c1"
    Then the sitting shows "Decided without you"
    And the sitting shows "Took the short path"

  Scenario: The changelog line shows
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    Then the sitting shows "Changelog: Added · Export notes as Markdown"

  Scenario: OK asks first and says what follows
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press o at the sitting
    Then the sitting shows "This runs, as you:"
    And the sitting shows "gdd resume c1 --g4 ok"
    And the sitting shows "The change is promoted."
    And the sitting shows "y to run · n to go back"
    And the record of "c1" is in state "in-pre"

  Scenario: y on OK closes the screen and gdd promotes the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press o at the sitting
    And I press y at the sitting
    Then the sitting is closed
    When the sitting runs what was chosen
    Then it succeeds
    And the record of "c1" has the gate "G4" with decision "ok" by "human"
    And the record of "c1" is in state "closed"

  Scenario: KO with the human's words
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press k at the sitting
    And I type "Not what I asked" at the sitting
    And I press Enter at the sitting
    Then the sitting shows "gdd resume c1 --g4 ko --note"
    And the sitting shows "Not what I asked"
    And the sitting shows "The change is rejected and stays out of production."
    When I press y at the sitting
    And the sitting runs what was chosen
    Then it succeeds
    And the record of "c1" has the gate "G4" with decision "ko" by "human"
    And the record of "c1" is in state "rejected"

  Scenario: Polish with the human's words runs one more round
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press p at the sitting
    And I type "Shorter error messages" at the sitting
    And I press Enter at the sitting
    Then the sitting shows "gdd resume c1 --g4 polish --note"
    And the sitting shows "One more Builder round with your words, then the gates and the demo again."
    When I press y at the sitting
    And the sitting runs what was chosen
    Then it succeeds
    And the output contains "Paused at G4"

  Scenario: KO and polish need words
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press k at the sitting
    And I press Enter at the sitting
    Then the sitting shows "What should change?"
    And the sitting does not show "This runs"

  Scenario: n goes back and nothing runs
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press o at the sitting
    And I press n at the sitting
    Then the sitting shows "Reviewer: OK"
    And the sitting chose nothing

  Scenario: q closes the sitting and nothing runs
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press q at the sitting
    Then the sitting is closed
    And the sitting chose nothing
    And the record of "c1" is in state "in-pre"

  Scenario: A change decided meanwhile is not decided twice
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I run "gdd resume c1 --g4 ok"
    And I press o at the sitting
    And I press y at the sitting
    Then the sitting shows "c1 is closed now: nothing was run"
    And the sitting chose nothing
