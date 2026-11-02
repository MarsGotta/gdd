@hidden
Feature: The report skill explains the costs as tminus shows them (hidden examples)

  Background:
    Given an empty folder

  Scenario: OpenCode's report command explains them too
    When I run "tminus init --name Shop --platform cli --stack python --agents claude opencode"
    Then it succeeds
    And the file ".opencode/commands/tminus-report.md" contains "unknown means the agent reported no cost and its model has no price there"
    And the file ".opencode/commands/tminus-report.md" does not contain "shows cost 0"
