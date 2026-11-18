Feature: <capability, as the person names it>

  Rule: <one behavior the person gave or approved>

    @brief:<id> @id:<capability>-<n>
    Scenario: <the case that checks this rule>
      Given <the starting state with concrete values>
      When <someone takes one action>
      Then <the outcome and where each effect can be seen>

  @assumed
  Rule: <one behavior the agent chose, stated as a fact>

    @brief:<id> @id:<capability>-<n>
    Scenario: <the case that checks this assumption>
      Given <the starting state with concrete values>
      When <someone takes one action>
      Then <the outcome and where each effect can be seen>

  @question
  Rule: <what can a later change decide?>
