Feature: The report skill explains the costs as tminus shows them
  tminus report marks a cost estimated from the prices of tminus/models.toml with ~ and says unknown,
  naming the models, for calls whose agent reported no cost and whose model has no price. It
  never shows a cost of 0 for them. The report skill the person's agent runs explains those two
  marks, and that with a subscription the cost is notional: tokens are what compares.

  Background:
    Given an empty folder

  Scenario: The report skill explains estimated and unknown costs
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file ".claude/skills/tminus-report/SKILL.md" contains "A cost marked ~ is an estimate from the prices in tminus/models.toml"
    And the file ".agents/skills/tminus-report/SKILL.md" contains "unknown means the agent reported no cost and its model has no price there"
    And the file ".claude/skills/tminus-report/SKILL.md" does not contain "shows cost 0"
