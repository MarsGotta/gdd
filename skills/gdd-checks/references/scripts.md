# The project's own scripts

When the ladder reaches its third step, the script is a check like any other: `gdd-checks` writes it by these rules, and `gdd-review` judges it by them.

- **One job,** written from the scenarios of its gate, with the bad case as its first test.
- **Deterministic:** the same input gives the same output, with no network, no clock and no randomness.
- **Never interactive.** Everything comes in through arguments, environment variables or stdin, and `--help` says what it checks, how to run it and which scenarios it covers.
- **Exit code 0 when it passes,** and a documented code for each kind of failure. When it fails, it names the file, the line and the rule, so it fails for the right reason.
- **Results to stdout,** structured when something else reads them, and diagnostics to stderr. The output stays short enough that an agent's environment doesn't cut it.
- **It handles its own errors** instead of leaving them to the agent, and every value comes from the blueprint or says why it has that value.
- **Only the languages and tools the project already uses.** A new dependency is pinned, and goes through the ladder.
- **It reads only what it needs,** writes nothing outside its output and never touches secrets.
- **Simple,** with nothing it doesn't need yet. It's sealed and protected like any other check, and it runs with the same command locally and in CI, a command the constitution records.
