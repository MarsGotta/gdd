# Quickstart

On this page you take one change of a small example shop from an empty folder to a release in
production. You use a terminal to install gdd and create the
project; after that, the work goes through your coding agent.

<!-- Every output on this page comes from one real run of gdd 0.1.0 (main at e244577) on
2026-10-31, with Claude Code as the chat agent and Codex as the Reviewer, in a throwaway
project `shop`. The whole run, from the form to G4, took about eight minutes. -->

## Before you start

- Linux or macOS, with [git](https://git-scm.com) and [Rust](https://rustup.rs) 1.98 or
  newer. Rust builds gdd.
- uv, OSV-Scanner and gitleaks, the tools that the Python stack of this example runs.
  [Install gdd](../how-to/install.md#python) links each one.
- At least one coding agent installed and signed in: Claude Code, Codex CLI or OpenCode. Two
  of different model families are better, one to build and one to review, because models of one
  family share blind spots.
- git knows your name and email (`git config user.name` and `git config user.email`):
  `gdd init` commits what it writes.

## 1. Install gdd

Install the `gdd` binary and check it:

```bash
cargo install --locked --git https://github.com/MarsGotta/gdd --tag v0.1.0 gdd
gdd --version
```

You should see:

```text
gdd 0.1.0
```

If your shell cannot find `gdd`, `~/.cargo/bin` is not on your `PATH`: see
[Install gdd](../how-to/install.md).

## 2. Run gdd init

In a new folder with a git repository, run `gdd init`:

```bash
mkdir shop
cd shop
git init -b main
gdd init
```

A form opens in the terminal. It asks seven questions, one per step, and writes nothing until
the end. You should see the first step:

```text
┌ gdd init · Step 1 of 7 · Project name ───────────────────────────────────────────────────────────┐
│What is the project called?                                                                       │
│> █                                                                                               │
│Enter: next · Ctrl-C: quit                                                                        │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Answer each step and press `Enter`. Lists move with the arrow keys, and `Space` checks a box.
The last step is the summary:

```text
┌ gdd init · Summary ──────────────────────────────────────────────────────────────────────────────┐
│Every answer, before anything is written.                                                         │
│Name: shop                                                                                        │
│Description: A small online shop                                                                  │
│Platform: web                                                                                     │
│Stacks: python                                                                                    │
│Agents: claude, codex                                                                             │
│Language: en                                                                                      │
│Sensitive: Losing a customer's order                                                              │
│Enter to set up · Esc to go back                                                                  │
│Ctrl-C: quit                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Press `Enter`. gdd writes the project and commits it. Then it installs the stack and commits
its lock file, the file that pins the version of every dependency. You should see, at the end:

```text
gdd/project.toml
gdd/commands.toml
...
.agents/skills/gdd-stack-python/SKILL.md
python: uv lock && uv sync --locked
...
Installed 39 packages in 61ms
...
Committed: uv.lock
Next: start your coding agent here and run the gdd-setup command (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).
```

Look at the last line: it tells you where to go next. If the output stops with an error after
the `python:` line, uv is probably missing.

From here on the work goes through your agent. The Board and the sitting, further down, are
windows you open in a terminal to watch.

## 3. Run /gdd-setup in your agent

Start your agent in the `shop` folder and run the setup skill. In Claude Code and OpenCode:

```text
/gdd-setup
```

In Codex, skills start with `$`: `$gdd-setup`, and later `$gdd-change`. If the agent does not
know the skill, you started it outside the `shop` folder.

The agent reads what the form wrote and asks only what is missing:

```text
Setup already has the basics: a web shop in Python, scenarios run with pytest-bdd, a skeleton
that runs, and one sensitive area (losing a customer's order). I need a few answers before I
write the Brief:

1. **What is the shop, in a few lines?** ...
2. **Who is it for?** ...
3. **Web framework.** I recommend **Django**. ... The alternative is **FastAPI**, if you'd
   rather have an API with a separate frontend.
4. **What else would be serious if it failed?** ...
```

Answer in your own words, briefly. The agent writes the Project Brief and runs the self-test of
the [harness](../glossary.md#harness), the files and commands gdd uses to check your project.
Then it asks you to approve the *red zones*, the paths that always get the strictest checks:

```text
The harness self-test passes: **harness 4/4**. ...

**1. Approve the red zones.** ... For now there are only two:
- **The harness itself:** gdd's own files, the CI workflow and the agent instructions.
- **Dependencies and the toolchain:** the files that decide which outside libraries the shop
  uses.

"Losing a customer's order" is in the Brief for now. It becomes a red zone once there is real
order-storage code. Reply **"zones OK"** to approve.
...
```

Say `zones OK`. The agent opens the Setup Gate (G0) and ends with:

```text
Setup is done and **G0 is open**. The harness is ready: the self-test passes all four checks,
and the red zones you approved are recorded.
...
**Next step:** I recommend `/gdd-discover` before the first change. ... If you'd rather start
building now, run `/gdd-change` with one sentence, ...
```

Notice **G0 is open**: the project is ready for its first change. The agent recommends
`/gdd-discover`, for bigger ideas; for one small change, go straight to the next step.

## 4. Describe your first change with /gdd-change

Say what you want in a sentence:

```text
/gdd-change show the cart total, taxes included
```

The Orchestrator, the role your agent plays here, creates the change `cart-total`. It writes
its *scenarios*, examples in Gherkin (plain Given, When, Then sentences) of what must happen,
and shows them to you with the change's color:

```text
I've drafted the scenarios for **cart-total**. It's yellow, so there's no plan for you to
approve and the red zones are unchanged. Before I commit them I need your OK and three answers.

**The scenarios** (all prices are set by you, tax already included):

> **Background:** the tax rate is 21%
>
> **1. The total adds up every piece in the cart**
> A cart with 1 × Blue mug at 24.20 and 2 × Wide bowl at 36.30.
> The total is **96.80**, and the tax included in it is **16.80**.
...
**Next step:** reply "OK" if all my recommendations stand, or give your rate, currency and any
corrections. I'll then commit the scenarios, approve G1 with your note, and launch the build.
```

Notice *yellow*, the color of ordinary code: a Reviewer of another model family reads the
change, and you meet it twice, now and at the end.

## 5. Approve the scenarios

Answer OK, or what to change, with the answers to its questions. Your OK is the Scenario Gate
(G1), and it seals the scenarios as the contract the code must meet:

```text
OK, as they are. My prices include tax, 21% is my rate and I sell in euros.
```

The agent records G1 and launches the change:

```text
The **cart-total** change is now building on its own. I'll get a notification when it finishes
or stops.
...
**Next step:** wait for the build. When it pauses at G4, I'll show you the result in plain
words and you answer OK, KO, or "polish" with your notes.
```

The Builder now writes the tests and the code, the gates check it and the Reviewer reads it.
To watch the change move, open the Board in a terminal:

```bash
gdd board
```

You should see the change under Build:

![The Board of shop: Build (1), cart-total, yellow, building, G1:ok](../img/qs-board-build.png)

The Board does not refresh by itself. To see where the change is now, press `q` and open it
again.

## 6. Approve at G4

After about two minutes the change pauses at the Release Gate (G4), merged into `main` and
deployed to pre, the environment before production. The agent comes back to the chat:

```text
The **cart-total** change is built and checked, and it's waiting for your **G4** answer.

**What it does:** a cart adds up the pieces in it and shows two figures, the total and how much
of that total is tax.
...
**Checks:**
- **All 6 scenarios pass**, including the 2 hidden examples the Builder never saw. ...
- **The Reviewer from another model family said OK** with no findings.
...
**Next step:** reply **OK** to send it to production (simulated for now), **KO** to reject it,
or **polish** with what you'd like changed.
```

Look at the checks first. The six scenarios are the three you approved, two *hidden examples*
the Builder never saw, and the smoke scenario Setup wrote to check that the project runs.

The G4 page of the sitting, `gdd sitting cart-total`, shows the same decision:

![The G4 sitting of cart-total: how it works, Reviewer OK, no demo, decided without you](../img/qs-sitting-g4.png)

Say OK:

```text
OK
```

The agent promotes the change and closes it:

```text
**cart-total** is released as **v0.1.0**, and the change is closed. Production is still
simulated, so nothing is live on the web yet. The shop's core now has a cart that shows the
total and the tax included in it.
...
```

`main` now holds the change: the two commits of its branch, the scenarios and the work, and the
merge commit that joined them, tagged `v0.1.0`. Opened again, the Board shows it under Done:

![The Board of shop: Done (1), cart-total, yellow, closed, G1:ok, G3:ok, G4:ok](../img/qs-board-done.png)

If a change stops on the way instead, it *parks*: the agent tells you why and proposes the next
step ([Get a parked change going again](../how-to/parked-change.md)).

You installed gdd, set up a project with your agent, approved one change's scenarios and its
result, and released it as `v0.1.0`.

## Next steps

- [Your first change](tutorial.md): the same path with one real change, every screen shown.
- [Describe a change to your agent](../how-to/run-a-change.md): sizes, types and what the
  Orchestrator asks you.
- [Skills reference](../reference/skills/index.md): every skill you can run from your agent.
- [GDD in one page](../explanation/method.md): why the gates are where they are.
