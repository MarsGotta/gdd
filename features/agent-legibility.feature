Feature: Agents can tell when to use each tminus skill and how to check the project
  A coding agent picks a skill by its description alone, so every skill and command tminus installs
  says what it does and when to use it, as the Agent Skills specification asks: the ones the
  person's agent runs start with "Use when", and the four tminus starts itself headless (build,
  build-test, review, replan) say that tminus starts them and that nobody should run them by hand.
  The AGENTS.md that tminus init writes tells agents what they cannot see in the code: how to
  install, the check commands of the project's stacks, how to run the scenarios of one file, and
  that tminus g2 and tminus selftest are what counts. An AGENTS.md the project already has stays as it
  is. A project set up earlier gets the new descriptions with tminus agents install.

  Background:
    Given an empty folder

  Scenario Outline: Each skill the person's agent runs says when to use it
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex opencode"
    Then it succeeds
    And the file ".claude/skills/tminus-<skill>/SKILL.md" contains "description: 'Use when <when>"
    And the file ".agents/skills/tminus-<skill>/SKILL.md" contains "description: 'Use when <when>"
    And the file ".opencode/commands/tminus-<skill>.md" contains "description: 'Use when <when>"

    Examples:
      | skill    | when                                                  |
      | setup    | the project was just set up with tminus init             |
      | discover | the person wants to explore or plan before building   |
      | change   | the person asks for a feature, a fix, a refactor      |
      | status   | a session starts                                      |
      | report   | the person asks what the agents cost                  |
      | hotfix   | something is broken in production                     |
      | learn    | several changes have closed or parked                 |

  Scenario Outline: A skill tminus starts itself says nobody should run it by hand
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex opencode"
    Then it succeeds
    And the file ".claude/skills/tminus-<skill>/SKILL.md" contains "description: 'Started by tminus; do not run it yourself."
    And the file ".claude/skills/tminus-<skill>/SKILL.md" contains "<role>"
    And the file ".opencode/commands/tminus-<skill>.md" contains "description: 'Started by tminus; do not run it yourself."

    Examples:
      | skill      | role                                       |
      | build      | headless as the Builder in build mode      |
      | build-test | headless as the Builder in test mode       |
      | review     | headless as the Reviewer                   |
      | replan     | headless as the Orchestrator               |

  Scenario: The instructions of a stack say when to read them
    When I run "tminus init --name Notes --platform cli --stack python --agents claude codex"
    Then it succeeds
    And the file ".claude/skills/tminus-stack-python/SKILL.md" contains "description: 'Use when writing or reviewing Python code"
    And the file ".agents/skills/tminus-stack-python/SKILL.md" contains "pytest-bdd scenario steps in this GDD project"

  Scenario: A project set up earlier gets the new descriptions from tminus agents install
    Given I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    And the file ".claude/skills/tminus-change/SKILL.md" says "description: an older description of the change skill"
    When I run "tminus agents install"
    Then it succeeds
    And the file ".claude/skills/tminus-change/SKILL.md" contains "description: 'Use when the person asks for a feature"
    And the file ".claude/skills/tminus-change/SKILL.md" does not contain "an older description of the change skill"

  Scenario: The AGENTS.md of a Python project names its install and check commands
    When I run "tminus init --name Notes --platform cli --stack python --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "`tminus` runs the gates"
    And the file "AGENTS.md" contains "## Install, checks and scenarios"
    And the file "AGENTS.md" contains "- Install: `uv sync --locked` (the first time, `tminus bootstrap` writes the lock file)."
    And the file "AGENTS.md" contains "- `types`: `uv run mypy src`"
    And the file "AGENTS.md" contains "- `unit`: `uv run pytest -q tests/unit`"
    And the file "AGENTS.md" contains "export TMINUS_FEATURES=features/<name>.feature"
    And the file "AGENTS.md" contains "`tminus g2 <change>`"
    And the file "AGENTS.md" contains "`tminus selftest`"
    And in the file "AGENTS.md", "## Install, checks and scenarios" comes before "- `unit`: `uv run pytest -q tests/unit`"

  Scenario: The AGENTS.md of a Rust project names only the commands its stack has
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "- Install: `cargo fetch --locked`"
    And the file "AGENTS.md" contains "- `unit`: `cargo test --workspace`"
    And the file "AGENTS.md" contains "- `acceptance`: `cargo test --test acceptance`"
    And the file "AGENTS.md" does not contain "- `types`:"
    And the file "AGENTS.md" does not contain "uv run"

  Scenario: The AGENTS.md of a project with two stacks names the commands of each
    When I run "tminus init --name Notes --platform cli --stack python rust --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "- Install: `uv sync --locked && cargo fetch --locked`"
    And the file "AGENTS.md" contains "- `unit-python`: `uv run pytest -q tests/unit`"
    And the file "AGENTS.md" contains "- `unit-rust`: `cargo test --workspace`"

  Scenario: Agents tminus started are told to run only what their command allows
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "If tminus started you (`TMINUS_ROLE` is set), run only what your command allows"
    And the file "AGENTS.md" contains "when the two differ, the file wins"

  Scenario: An AGENTS.md the project already has stays as it is
    Given the file "AGENTS.md" says "Our rules"
    When I run "tminus init --name Notes --platform cli --stack python --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "Our rules"
    And the file "AGENTS.md" does not contain "Install, checks and scenarios"
    And the output contains "add to AGENTS.md"

  Scenario: The setup skill keeps AGENTS.md in step with the gate commands
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file ".claude/skills/tminus-setup/SKILL.md" contains "under Install, checks and scenarios in `AGENTS.md`"
