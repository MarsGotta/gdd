# Environment variables

Every environment variable gdd reads, every one it sets for the commands and agents it starts,
and what it strips from the agents' environment.

## What gdd reads

| Variable | Read by | Meaning |
|---|---|---|
| `GDD_CHANGE` | `gdd classify`, `snapshot`, `inbox`, `prepare`, `merge`, `release`, `g2`, `g3`, `guard` | The change id when the command line gives none. Empty counts as unset. |
| `GDD_ROLE` | every command | The role the run is for. A value that starts with `builder` or `reviewer` makes `gdd gate`, `resume`, `release`, `changelog-line`, `selftest`, `learn` and `upgrade` refuse. With any value, `gdd g2` and `gdd g3` do not log their run. Unset: a person. |
| `GITHUB_HEAD_REF` | `gdd ci` | The branch to check, as GitHub Actions sets it on a pull request; without it, the branch checked out. |
| `HOME` | `gdd session` | Where the Claude Code transcripts are: `$HOME/.claude/projects/<project root>`, every character of the root outside `A-Za-z0-9` turned into `-`. |
| `PATH` | `gdd init`, `gdd launch`, `gdd resume` | Where the agents' programs are (`claude`, `codex`, `opencode`): the first absolute folder that has the program, executable. `gdd init` without `--agents` sets up the agents it finds there. |

gdd does not read `GDD_ROOT` or `GDD_STATE`: it finds the project root by looking for
`gdd/project.toml` from the folder it runs in upwards, and the state root by asking git for the
main checkout.

Every other variable passes through: gate commands, release steps, `gdd bootstrap` and git run
in the environment gdd runs in, so git's own variables (`GIT_AUTHOR_NAME` and the rest) say who
commits.

`bin/gdd`, the script `gdd init` writes, reads one more:

| Variable | Meaning | Example |
|---|---|---|
| `GDD_BIN` | The gdd program `bin/gdd` runs; `gdd` on the `PATH` without it. | `GDD_BIN=/home/me/.cargo/bin/gdd bin/gdd status` |

```bash
GDD_CHANGE=cart-total bin/gdd g2 --role builder-build
GITHUB_HEAD_REF=change/cart-total bin/gdd ci
```

## What gate commands get

Each command of `[g2]` and `[g3]` runs with gdd's environment plus four variables. Lists hold
one path per line, from the project root.

| Variable | Value |
|---|---|
| `GDD_CHANGE` | The change id. |
| `GDD_COLOR` | Its color: `green`, `yellow` or `red`. |
| `GDD_DIFF_FILES` | What the change touched against the main branch; its planned paths while it touched nothing. Mutation commands test only these. |
| `GDD_FEATURES` | The change's scenario files: the `.feature` files needing G1 it plans or touched, and the ones its record sealed at G1, hidden examples included. Empty when it has none. |

`gdd ci` gives its gate commands the same four. Other runs set them differently:

| Run | Variables |
|---|---|
| `gdd adopt`, `gdd learn` | `GDD_CHANGE`, `GDD_COLOR`, `GDD_DIFF_FILES` and `GDD_FEATURES`, all empty. |
| `gdd selftest` | `GDD_CHANGE=selftest`. |
| `gdd bootstrap` | None added. |

## What release steps get

Each step of `[release]` runs with gdd's environment plus:

| Variable | Value |
|---|---|
| `GDD_CHANGE` | The change id. |
| `GDD_VERSION` | Once the change has a version (`[versions] mode = "auto"`, a changelog line, at `promote`): the version without its `v`, such as `0.1.0`. Unset before. |

```toml
promote = "./scripts/promote.sh \"$GDD_VERSION\""
```

## Variables the scenario runners read

The runners of the [stack skeletons](stacks.md#scenarios-and-hidden-examples) read two
variables; gdd sets the first, and the G3 commands set both.

| Variable | Meaning |
|---|---|
| `GDD_FEATURES` | The `.feature` files to run, one per line; every one when empty. |
| `GDD_HIDDEN` | `1`: also run `features/hidden/`. Python and Rust only; TypeScript leaves out scenarios tagged `@hidden` in G2 instead. |

```toml
acceptance_full = "GDD_FEATURES= GDD_HIDDEN=1 uv run pytest -q tests/acceptance || [ $? -eq 5 ]"
```

## What agents get

gdd starts each agent's own program, never through a shell, from the change's worktree, with
nothing on its standard input and its standard error discarded. Its environment is gdd's own,
cleared first and then rebuilt:

1. Every variable of gdd's environment, except those that
   [hold a secret](#which-variables-hold-a-secret), by their name or by their value.
2. Those stripped variables that `[env] pass` of `gdd/models.toml` names exactly.
3. Four variables of its own:

| Variable | Value |
|---|---|
| `GDD_CHANGE` | The change id. |
| `GDD_ROOT` | The project root: the change's worktree, where the agent runs. |
| `GDD_STATE` | The state root: the main checkout, whose `.gdd/` holds the change's records. The agents' commands tell them to read the state there. |
| `GDD_ROLE` | `builder-test`, `builder-build`, `reviewer` or `orchestrator`. |

### Which variables hold a secret

A variable holds a secret, and is stripped, in two cases.

**Its name says so.** gdd splits the name into words at `_`, `-` and `.`, and reads them in any
case. When one word is `KEY`, `KEYS`, `TOKEN`, `TOKENS`, `SECRET`, `SECRETS`, `PASSWORD`,
`PASSWORDS`, `PASSWD`, `PASS`, `CREDENTIAL`, `CREDENTIALS`, `APIKEY` or `DSN`, the variable is
stripped. Only whole words count:

| Name | Its words | Reaches the agent |
|---|---|---|
| `OPENAI_API_KEY`, `XAPI_KEY`, `API_KEY_FILE` | `KEY` | no |
| `TOKEN`, `github_token`, `MY_TOKENS`, `npm-tokens` | `TOKEN`, `TOKENS` | no |
| `AWS_SECRET_ACCESS_KEY`, `client.secret` | `SECRET`, `KEY` | no |
| `Db_Password`, `MYSQL_PASSWD`, `DB_PASS`, `PASS_FILE` | `PASSWORD`, `PASSWD`, `PASS` | no |
| `GOOGLE_APPLICATION_CREDENTIALS`, `MAPS_APIKEY`, `SENTRY_DSN` | `CREDENTIALS`, `APIKEY`, `DSN` | no |
| `KEYBOARD_LAYOUT`, `TOKENIZER_PATH`, `PASSAGE_DIR`, `MONKEY`, `APITOKEN` | no such word | yes |
| `PATH`, `HOME`, `PWD`, `EDITOR` | no such word | yes |

**Its value says so.** A value that is a URL with a password in its user part,
`scheme://user:password@host` or `scheme://:password@host`, holds a secret, whatever the name:

| Value | Reaches the agent |
|---|---|
| `postgres://app:PASSWORD@db.example/app`, `redis://:PASSWORD@cache:6379` | no |
| `https://example.com/docs`, `http://localhost:8080`, `https://me@host` (no password) | yes |

So `DATABASE_URL=postgres://app:PASSWORD@db.example/app` never reaches an agent, while
`DOCS_URL=https://example.com/docs` does.

### Pass a variable on purpose

`[env] pass` in `gdd/models.toml` lists the variables the agents get even though they hold a
secret, by their exact name:

```toml
[env]
pass = ["MM_API_KEY", "DATABASE_URL"]
```

> [!WARNING]
> A secret that neither its name nor its value gives away (a raw key in `STRIPE_LIVE`, or in
> `APITOKEN`, which is one word) reaches the agents. Keep production credentials out of the
> environment you launch changes from.

The agents use their own logins (subscriptions), not keys gdd passes on. A `bin/gdd` command an
agent runs inherits the agent's environment, so its gate commands do not see the stripped
variables either.

## See also

- [Roles](roles.md): what each `GDD_ROLE` may do.
- [models.toml](configuration.md#modelstoml): `[env] pass` and the other agent settings.
- [Keeping agents honest](../explanation/safety.md): why secrets stay away from agents.
- [Run the gates in CI](../how-to/ci.md): `gdd ci` and `GITHUB_HEAD_REF`.
