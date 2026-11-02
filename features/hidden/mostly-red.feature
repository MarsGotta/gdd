@hidden
Feature: When almost every change comes out red, the report says the blueprint marks too much as delicate (hidden examples)

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

  Scenario: Only the last ten closed changes count, the older ones do not
    Given the log has these changes, one a day from 2026-09-20:
      | change | color  | state  |
      | p1     | yellow | closed |
      | p2     | green  | closed |
      | p3     | yellow | closed |
      | p4     | yellow | closed |
      | p5     | green  | closed |
      | p6     | red    | closed |
      | p7     | red    | closed |
      | p8     | yellow | closed |
      | p9     | red    | closed |
      | p10    | red    | closed |
      | p11    | red    | closed |
      | p12    | red    | closed |
      | p13    | green  | closed |
      | p14    | red    | closed |
      | p15    | red    | closed |
      | p16    | red    | closed |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Most changes come out red: 8 of the last 10 closed."
    And the output contains "If almost everything is red, the blueprint is wrong: the zone map may mark too much as delicate."

  Scenario: Changes that did not close do not count
    Given the log has these changes, one a day from 2026-09-28:
      | change | color  | state       |
      | q1     | red    | closed      |
      | q2     | red    | in progress |
      | q3     | red    | closed      |
      | q4     | red    | closed      |
      | q5     | -      | closed      |
      | q6     | red    | closed      |
      | q7     | red    | closed      |
      | q8     | yellow | rejected    |
      | q9     | red    | closed      |
      | q10    | red    | closed      |
      | q11    | red    | closed      |
      | q12    | red    | closed      |
      | q13    | yellow | in progress |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "All: 13 change(s) · 10 closed"
    And the output contains "Most changes come out red: 9 of the last 10 closed."

  Scenario: Fewer than ten closed changes are too few to judge
    Given the log has these changes, one a day from 2026-10-05:
      | change | color | state  |
      | r1     | red   | closed |
      | r2     | red   | closed |
      | r3     | red   | closed |
      | r4     | red   | closed |
      | r5     | red   | closed |
      | r6     | red   | closed |
      | r7     | red   | closed |
      | r8     | red   | closed |
      | r9     | red   | closed |
      | r10    | red   | closed |
    When I run "tminus report --promises"
    Then it succeeds
    And the output contains "Most changes come out red: 10 of the last 10 closed."
    When I run "tminus report --promises --since 2026-10-06"
    Then it succeeds
    And the output contains "All: 9 change(s) · 9 closed"
    And the output does not contain "Most changes come out red"
