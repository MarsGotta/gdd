Feature: The setup, status and learn skills start from what the project has
  The Setup form (or the flags of tminus init) already wrote the project basics and a Brief
  skeleton, so the tminus-setup skill reads them first and asks the person only what they leave
  out. It installs a stack whose lock file is missing with tminus bootstrap, and in a repository
  that already has code it runs tminus adopt before the self-test, so old lint or test debt does
  not keep G0 closed. The tminus-status skill reads only files every project has: AGENTS.md, not
  GDD.md, which tminus init does not write. The tminus-learn skill runs tminus learn, which lists the
  polish notes and the Reviewer findings that repeat.

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: The setup skill starts from what tminus init wrote
    Then the file ".claude/skills/tminus-setup/SKILL.md" contains "Read `tminus/project.toml` and `docs/brief.md` first"
    And the file ".claude/skills/tminus-setup/SKILL.md" contains "ask only what they leave out"

  Scenario: The setup skill installs a stack without a lock file
    Then the file ".claude/skills/tminus-setup/SKILL.md" contains "run `tminus bootstrap`"

  Scenario: The setup skill adopts a repository that already has code
    Then the file ".claude/skills/tminus-setup/SKILL.md" contains "run `tminus adopt` before `tminus selftest`"

  Scenario: The status skill reads only what every project has
    Then the file ".claude/skills/tminus-status/SKILL.md" contains "read `AGENTS.md`"
    And the file ".claude/skills/tminus-status/SKILL.md" does not contain "GDD.md"

  Scenario: The learn skill runs tminus learn
    Then the file ".claude/skills/tminus-learn/SKILL.md" contains "Run `tminus learn`"
