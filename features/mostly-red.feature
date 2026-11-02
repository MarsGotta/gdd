Feature: When almost every change comes out red, the report says the blueprint marks too much as delicate
  GDD is not ceremony: red is for what is really delicate, and a red change stops you twice, at
  the sitting and at G4. If almost every change comes out red, the blueprint is wrong, not the
  changes: the zone map probably marks too much as delicate. `tminus report --promises` looks at
  the last 10 changes that closed in the period it reads, in the order they were created, and when
  at least 8 of them came out red it says so in plain words. It is a warning: it stops nothing and
  changes no color. A change's color is the one its last build started with, as the rest of the
  report counts it, whether the zone map or the change's declared minimum made it red. Changes
  that did not close do not count, and fewer than 10 closed changes are too few to judge: the
  report says nothing.

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

  Scenario: Eight of the last ten closed changes came out red: the report says the zone map may mark too much
    Given the log has these changes, one a day from 2026-10-01:
      | change | color  | state  |
      | a      | red    | closed |
      | b      | red    | closed |
      | c      | yellow | closed |
      | d      | red    | closed |
      | e      | red    | closed |
      | f      | red    | closed |
      | g      | green  | closed |
      | h      | red    | closed |
      | i      | red    | closed |
      | j      | red    | closed |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Most changes come out red: 8 of the last 10 closed."
    And the output contains "If almost everything is red, the blueprint is wrong: the zone map may mark too much as delicate."

  Scenario: Seven of ten is not almost every change: the report says nothing about the zone map
    Given the log has these changes, one a day from 2026-10-01:
      | change | color  | state  |
      | a      | red    | closed |
      | b      | red    | closed |
      | c      | yellow | closed |
      | d      | red    | closed |
      | e      | red    | closed |
      | f      | yellow | closed |
      | g      | green  | closed |
      | h      | red    | closed |
      | i      | red    | closed |
      | j      | red    | closed |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "All: 10 change(s) · 10 closed"
    And the output does not contain "Most changes come out red"

  Scenario: Only the last ten closed changes count, the older ones do not
    Given the log has these changes, one a day from 2026-10-01:
      | change | color  | state  |
      | a      | red    | closed |
      | b      | red    | closed |
      | c      | red    | closed |
      | d      | red    | closed |
      | e      | red    | closed |
      | f      | red    | closed |
      | g      | yellow | closed |
      | h      | red    | closed |
      | i      | yellow | closed |
      | j      | red    | closed |
      | k      | red    | closed |
      | l      | yellow | closed |
      | m      | red    | closed |
      | n      | red    | closed |
      | o      | red    | closed |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "All: 15 change(s) · 15 closed"
    And the output does not contain "Most changes come out red"

  Scenario: Changes that did not close do not count
    Given the log has these changes, one a day from 2026-10-01:
      | change | color  | state       |
      | a      | red    | closed      |
      | b      | yellow | closed      |
      | c      | red    | closed      |
      | d      | red    | rejected    |
      | e      | red    | closed      |
      | f      | yellow | closed      |
      | g      | red    | closed      |
      | h      | green  | closed      |
      | i      | red    | closed      |
      | j      | red    | closed      |
      | k      | red    | closed      |
      | l      | red    | in progress |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "All: 12 change(s) · 10 closed"
    And the output does not contain "Most changes come out red"

  Scenario: Fewer than ten closed changes are too few to judge
    Given the log has these changes, one a day from 2026-10-01:
      | change | color | state       |
      | a      | red   | closed      |
      | b      | red   | closed      |
      | c      | red   | closed      |
      | d      | red   | closed      |
      | e      | red   | closed      |
      | f      | red   | closed      |
      | g      | red   | closed      |
      | h      | red   | closed      |
      | i      | red   | closed      |
      | j      | red   | in progress |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "All: 10 change(s) · 9 closed"
    And the output does not contain "Most changes come out red"

  Scenario: The warning reads the same period as the rest of the report
    Given the log has these changes, one a day from 2026-10-01:
      | change | color  | state  |
      | a      | red    | closed |
      | b      | red    | closed |
      | c      | red    | closed |
      | d      | red    | closed |
      | e      | red    | closed |
      | f      | red    | closed |
      | g      | red    | closed |
      | h      | red    | closed |
      | i      | yellow | closed |
      | j      | yellow | closed |
      | k      | yellow | closed |
      | l      | yellow | closed |
    When I run "tminus report --promises"
    Then it succeeds
    And the output does not contain "Most changes come out red"
    When I run "tminus report --promises --until 2026-10-10"
    Then it succeeds
    And the output contains "Most changes come out red: 8 of the last 10 closed."
