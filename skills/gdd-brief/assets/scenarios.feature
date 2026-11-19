Feature: <capability, as the person names it>

  Rule: <one behavior the person gives in their own words>

    @brief:<id> @id:<capability>-<n>
    Scenario: <the case that checks this rule>
      Given <the starting state with concrete values>
      When <someone takes one action>
      Then <the outcome and where each effect can be seen>

  @assumed
  Rule: <one behavior the agent chose, stated as a fact: a value, a limit, or when a rule of the product takes effect>

    @brief:<id> @id:<capability>-<n>
    Scenario: <the case that checks this assumption>
      Given <the starting state with concrete values>
      When <someone takes one action>
      Then <the outcome and where each effect can be seen>

  @question
  Rule: <what can a later change decide?>
