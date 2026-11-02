Feature: Levels by risk
  How much the gates ask of a change follows from its risk: its color, whether it brings new
  scenarios (a new lock) and its lane. The rules give each risk a level: the lock alone, the lock
  and the guard, or the lock, the guard and the human's approval. Under the rules sits a floor no
  setting lowers: in the most sensitive area every gate asks for the guard and the human's
  approval, the guard looks deep and mutation testing runs; and the rules that decide risk are
  always in that area, where no agent may change them. Today's color table is read as levels and
  decides exactly as before.

  Background:
    Given a project that follows the method, with the sensitive area "src/storage/**"

  Scenario Outline: Each change gets the level its rules give its risk
    When I run "tminus new-change c1 --title App --paths <paths> <resumable>"
    Then the JSON field "color" is "<color>"
    And the JSON field "lane" is "<lane>"
    And the JSON field "reviewer" is <guard>
    And the JSON field "g4" is <approval>
    And the JSON field "human_plan" is <plan>
    And the JSON field "mutation" is <mutation>
    And the JSON field "review" is "<depth>"
    And the JSON field "tests_first" is <first>

    Examples:
      | paths                            | resumable   | color  | lane | guard | approval | plan  | mutation | depth | first |
      | src/app.txt                      | --resumable | yellow | fast | true  | false    | false | true     | quick | true  |
      | src/app.txt                      |             | yellow | full | true  | false    | false | true     | quick | true  |
      | src/app.txt features/app.feature | --resumable | yellow | full | true  | true     | false | true     | quick | true  |
      | src/storage/db.txt               | --resumable | red    | full | true  | true     | true  | true     | deep  | true  |

  Scenario: In the most sensitive area the rules cannot leave the guard or the human's approval out
    Given the file "tminus/rules.toml" says:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]

      [color.red.full]
      tests_first = true
      """
    When a change that plans "src/storage/db.txt" is opened
    Then the change waits for the guard
    And the change waits for the human's approval
    And the JSON field "mutation" is true
    And the JSON field "review" is "deep"

  Scenario: The rules that decide risk are always in the most sensitive area
    Given main has the file "tminus/zones.toml" with:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["tminus/**"]
      reason = "Settings"

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    When a change that plans "tminus/rules.toml" is opened
    Then the change counts as sensitive
    And the change waits for the guard
    And the change waits for the human's approval

  Scenario: An agent cannot change the rules that decide risk, even where the rules file does not protect them
    Given the file "tminus/rules.toml" says:
      """
      [protected]
      all = ["bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And a change that plans "src/app.txt" is at work
    When the "Builder in build mode" changes the rules that decide risk
    Then the agent's work is refused for touching the rules that decide risk
