Feature: The tool identifies itself

  Scenario: Asking for the version
    Given the gdd tool
    When I ask for its version
    Then the answer starts with "gdd "
