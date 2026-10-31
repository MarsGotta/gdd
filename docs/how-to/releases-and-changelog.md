# Release versions and keep a changelog

Give each change one line in the changelog, the file that tells the people who use your project
what changed in each version. Then release versions automatically or in batches, and set up the
steps that take a change to production. Use this guide before your first release, or when you
want to change how versions are made.

## Before you start

- A GDD project with at least one change merged: see [Describe a change to your
  agent](run-a-change.md).
- You know the Release Gate (G4): see [Review a change at G4](release-gate.md).

## 1. Give each change its changelog line

Every change carries one line in its merge commit, as git trailers:

```text
Changelog: Added · The cart shows its total
Breaking: no
```

Without a line, the change's title goes under the section of its type: Feature to `Added`,
Hotfix to `Fixed`, Migration to `Changed`. Refactor, Spike and No-code changes are left out.
If the title is not the line you want, ask your agent to set a better one before the merge, or
set it yourself:

```bash
gdd changelog-line cart-total Added The cart shows its total
```

```text
Changelog line of cart-total: Added · The cart shows its total
```

The section is one of those of [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), a
common format for changelogs: `Added`, `Changed`, `Deprecated`, `Removed`,
`Fixed` or `Security`. Any other is refused:

```text
error: the section must be one of Added, Changed, Deprecated, Removed, Fixed, Security
```

Mark a change that breaks what worked before, or leave one out on purpose:

```bash
gdd changelog-line export-csv Changed Orders export as CSV instead of JSON --breaking
gdd changelog-line tidy-cart --none
```

The merge commit of `export-csv` then ends with:

```text
Changelog: Changed · Orders export as CSV instead of JSON
Breaking: yes
```

Builders and Reviewers cannot set the line. In gdd 0.1.0, if the words of a line start with
`--`, put a `--` before them:
[`gdd changelog-line CHANGE Changed -- --time is faster`](../reference/cli/changelog-line.md).

## 2. Bring CHANGELOG.md up to date

`CHANGELOG.md` is yours: gdd never removes or rewrites a word of it. In gdd 0.1.0,
[`gdd init`](../reference/cli/init.md) does not create it. Until the file exists, each line
lives only in the trailers of its merge commit, and an automatic version tags that merge
commit. To start the file, ask your agent to start `CHANGELOG.md`, or create it with
`gdd changelog --write` as shown below and commit it.

[`gdd changelog`](../reference/cli/changelog.md) compares the file with the lines the merge
commits on main carry, version by version, and shows what is missing. What no `v<semver>` tag
released yet goes under Unreleased. Without a file, it lists every line merged so far.

```bash
gdd changelog
```

You should see:

```text
Missing from CHANGELOG.md:

## [Unreleased]

### Added

- The cart shows its total <!-- gdd: 2dabea449211 -->

Nothing written: gdd changelog --write adds them.
```

To add what is missing, or create the file with the Keep a Changelog header, write it and
commit it yourself:

```bash
gdd changelog --write
git add CHANGELOG.md
git commit -m "Update CHANGELOG.md"
```

```text
CHANGELOG.md: 1 line(s) added, nothing else changed.
```

Each added line carries a comment naming its commit, so a line you reword later still counts
as there. A line with the same words counts too. In a terminal, `gdd changelog` without
`--write` asks before it writes.

If your file already has versions, gdd keeps their heading style:

| Your file has | gdd reads it as |
|---|---|
| `## [Unreleased]`, `## Unreleased` (any case) | the Unreleased section |
| `## [0.1.0] - 2026-10-30` | Keep a Changelog |
| `## 0.1.0 - 2026-10-30` | without brackets |
| `## v0.1.0` | with a `v` |
| `## [0.1.0](<compare link>) (2026-10-30)` | a heading with a link |

New versions follow the style of your newest version heading, and a file without `###`
sections gets plain lines.

## 3. Choose how versions are made

A version is a git tag `v<major>.<minor>.<patch>` that follows semantic versioning (semver), a
numbering where a patch fixes, a minor adds and a major breaks what worked. The next version
comes from the lines of the changes since the newest tag:

| Line | Next version |
|---|---|
| `Fixed`, `Security` | patch |
| `Added`, `Changed`, `Deprecated`, `Removed` | minor |
| any line with `--breaking` | major, or minor while the major is 0 |

The first version of a project is `0.1.0`. Pick one of two modes in `gdd/rules.toml`:

- **auto** (the default, with no `[versions]` section): every change with a changelog line gets
  its version when it is promoted to production. The promote step gets it in `GDD_VERSION`,
  and gdd makes the tag only once the promotion succeeds. If main tracks a `CHANGELOG.md`, its
  Unreleased section becomes the version in a `Release v<x.y.z>` commit, which the tag names.
  Without the file, the tag names the merge commit.
- **manual**: promotions make no tag; you release in batches with
  [`gdd release`](../reference/cli/release.md).

  ```toml
  [versions]
  mode = "manual"   # release in batches with gdd release
  ```

In auto mode, a change promoted in a project that tracks a changelog looks like this on main
(`Start CHANGELOG.md` is the commit that created the file):

```text
d78a276 Release v0.1.0
06118e8 Start CHANGELOG.md
2dabea4 gdd(cart-total): G3 Merge Gate · Cart total
```

## 4. Release by hand

`gdd release` without a step shows the next version of what main merged since its newest tag,
and releases nothing:

```bash
gdd release
```

```text
Next version: v0.2.0 (from v0.1.0)

### Changed

- **Breaking:** Orders export as CSV instead of JSON

### Security

- Repeated failed logins are slowed down

Nothing released: gdd release --yes releases it.
```

Release it:

```bash
gdd release --yes
```

```text
Released v0.2.0
```

This works in either mode. If main tracks `CHANGELOG.md`, gdd adds the missing lines and turns
Unreleased into `## [0.2.0] - <day>` in a commit of its own (`Release v0.2.0`). Then it makes an
annotated tag `v0.2.0`. Nothing is pushed: push the commit and the tag yourself.

To choose the version, add `--version`. Without `--yes` it shows what it would release:

```bash
gdd release --version 1.0.0
```

```text
Next version: v1.0.0 (from v0.2.0)

### Added

- Receipt

Nothing released: gdd release --yes releases it.
```

The version must come after the newest tag. `gdd release --version 0.1.0` in a project
already at `v0.1.0` says:

```text
error: the version must come after v0.1.0
```

With nothing merged since the newest tag, `gdd release` says `Nothing to release since v0.2.0`.
Builders and Reviewers cannot release.

## 5. Configure the release steps

After the merge, the recipe takes a change through these steps, each a command in the
`[release]` section of `gdd/commands.toml`:

```mermaid
flowchart TD
    M[Merge: build, deploy_pre] --> P[pre: tests_pre, demo]
    P --> G{G4 needed?}
    G -->|yes| H[The human answers G4]:::human
    G -->|no| R[promote]
    H -->|OK| R
    R --> O[observe]
    O -->|errors rise| B[rollback]:::red
    O -->|fine| C[closed]:::green
    classDef green fill:#d9f2e3,stroke:#2e8b57,color:#123
    classDef red fill:#fbdcdc,stroke:#c0392b,color:#311
    classDef human fill:#e6e0f8,stroke:#6a4fc0,color:#213
```

[`gdd init`](../reference/cli/init.md) writes them empty, with `simulated = true`: empty steps
pass and are logged as simulated until you have a real pre and production. Fill them in and set
`simulated = false`:

```toml
[release]
simulated = false
build = "uv build -q"                                       # build the artifact once
deploy_pre = "mkdir -p .gdd/pre && cp dist/*.whl .gdd/pre/"
tests_pre = "ls .gdd/pre/*.whl"
demo = "mkdir -p .gdd/changes/$GDD_CHANGE/demo && GDD_FEATURES= uv run pytest -q tests/acceptance > .gdd/changes/$GDD_CHANGE/demo/scenarios.txt"
promote = "./deploy.sh production $GDD_VERSION"             # the same artifact, to production
observe = "./check-error-rate.sh"                           # fails if errors rise
rollback = "./deploy.sh production previous"
rollback_pre = "./deploy.sh pre previous"                   # used by the self-test
```

`promote`, `observe` and `rollback` above call scripts of your own, and the rest is a working
example for a Python package. Promote the same artifact you built once: never rebuild it for
production. Each step may run for `command_seconds` of `[limits]`. To try a step, run it on its
own:

```bash
gdd release pre CHANGE
gdd release promote CHANGE
gdd release observe CHANGE
```

A change that needs the human G4 is never promoted without it
(`This change needs a human G4 before promotion.`). A promotion that fails leaves production
untouched and the change waiting at G4; the same OK retries it.

If `observe` fails, the rollback runs and the change asks for a Hotfix. With an `observe` that
printed `error rate up` and exited with 1:

```text
error: release.observe failed (exit code 1):
error rate up
--- end of output ---
error rate up
Rollback: ok
Production failed while observing: rolled back to the previous version. A hotfix is needed. Message in .gdd/changes/receipt/inbox.md
```

The change is now `rolled-back`, and its Decision Inbox says what you need to decide:

```text
## Needs your decision
- Production failed while observing: rolled back to the previous version. A hotfix is needed.
```

Every key is in [Configuration](../reference/configuration.md#commandstoml).

## 6. Check the changelog in CI (optional)

`gdd changelog --check` writes nothing and fails when the file lacks a line:

```text
error: CHANGELOG.md misses 1 line(s): gdd changelog shows them, gdd changelog --write adds them
```

It prints `CHANGELOG.md covers every change on main.` when nothing is missing. If you keep the
file by hand, add it as a step of your workflow on main: see [Run the gates in CI](ci.md).

## Next steps

- [Review a change at G4](release-gate.md): OK, KO or polish at G4, before promotion.
- [Run the gates in CI](ci.md): what blocks a change before it merges, in continuous
  integration (CI).
- [Measure the work](measure.md): deployment frequency and change fail rate (the share of
  deployments that fail) with `--dora`.
