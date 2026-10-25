@hidden
Feature: Hidden examples for the smoke scenario

  Scenario: The version line has no trailing space
    Given the gdd tool
    When I ask for its version
    Then the answer starts with "gdd 0"
