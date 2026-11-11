# The GDD skills

You ask your agent for a filter by size in the catalog. One agent writes down with you what has to pass, another builds the checks, a third does the work, and a reviewer that saw none of it runs the change and tries to make it fail. You hear about it when your approval is needed, and when it's in.

That's a lot to explain every time, so it's written down once, in seven skills. A skill is a file of instructions your agent loads when it needs it. These install nothing and carry no code: they use what your project already has, its tests, its linter, its CI and git.

> [!NOTE]
> Version 0.2, contract `gdd/1`. The install steps were tried on 2026-11-11 with Claude Code 2.1.319, Codex CLI 0.166.0 and Antigravity's `agy` 1.4.2. The scenarios in [`evals/`](../evals/) try the skills with a real model, and they don't cover every skill yet.

## The four moments

```mermaid
flowchart LR
    accTitle: The four moments of GDD
    accDescr: Prepare happens once, with you. Every change goes through Brief and Execute before it reaches your main branch. When something slips through, Harden adds one more check, which every later change has to pass.
    P["Prepare<br/>once, with you"]
    B["Brief<br/>what has to pass"]
    E["Execute<br/>build, gates, review"]
    M(["Your main branch"])
    H["Harden<br/>with you"]
    P --> B --> E --> M
    M -. "something slipped through" .-> H
    H -. "one more check" .-> E
    classDef you fill:#0969da,stroke:#0969da,color:#ffffff
    class P,H you
```

Prepare happens once, in a conversation with you. From then on every change goes through Brief and Execute, and you step in only where your blueprint says a person approves. Harden is for the day something gets past every gate, so that what it would have taken to catch it stays as a check.

| Step | You say | You end up with |
|---|---|---|
| [1. Install the skills](#1-install-the-skills) | One command | The seven skills in your agent |
| [2. Prepare your project](#2-prepare-your-project) | "Set GDD up in this project" | A blueprint you approved, and the gates of each zone |
| [3. Ask for a change](#3-ask-for-a-change) | The change, in your own words | The change on your main branch, and a log of how it got there |
| [4. Harden what slipped through](#4-harden-what-slipped-through) | What failed, and where you found it | A check that catches it from then on |

## Before you start

- **A git repository.** The checks are built from what it already has: its tests, its linter, its CI.
- **An agent that loads skills.** Claude Code, Codex or Antigravity.
- **Your agent's deepest model, for Prepare and Harden.** If you aren't on it, the orchestrator tells you before it starts.
- **Protected branches, if your platform has them.** The skills work without them, but then nothing stops the agent from touching what judges it ([what they don't guarantee](#what-they-dont-guarantee)).

## 1. Install the skills

Pick your agent. Each one reads its own manifest from this repository, and none of them brings code.

**Claude Code**, 2.1.275 or later:

```
/plugin install gdd --marketplace MarsGotta/gdd
```

**Codex:**

```
codex plugin marketplace add MarsGotta/gdd
codex plugin add gdd@gdd
```

**Antigravity:**

```
git clone https://github.com/MarsGotta/gdd
agy plugin install gdd
```

**By hand,** if you'd rather keep them inside your project: copy the seven folders whose names start with `gdd` into `.claude/skills/` for Claude Code, or `.agents/skills/` for Codex. For Claude Code, copy the subagents in [adapters/](adapters/) to `.claude/agents/` as well.

Then check it's there:

| Agent | Command | What you should see |
|---|---|---|
| Claude Code | `claude plugin list` | `gdd@gdd`, version 0.2.0, enabled |
| Codex | `codex plugin list` | `gdd@gdd`, installed and enabled, 0.2.0 |
| Antigravity | `agy plugin list` | `gdd (enabled, 0.2.0)`, with 7 skills |

From a plugin, the names carry its prefix on Claude Code and Codex: `gdd:gdd`, `gdd:gdd-build` and so on. You rarely type them, because `gdd` loads by itself and launches the rest.

## 2. Prepare your project

**You say:**

```
Set GDD up in this project
```

**What happens.** The orchestrator loads `gdd-prepare` and sits down with you to decide what is delicate in your project and which gates it needs.

1. **It proposes first.** You get the usual zones and gates for a project like yours, to correct what doesn't fit. It asks only what changes a zone or a gate: money, accounts and permissions, personal or regulated data, the stack, what can't be undone. It also asks, before one happens, what an emergency may skip and who opens that gate.
2. **You mark the zones.** A zone is a set of paths with a color, by how much damage a change there can do: green, yellow or red. Anything no zone covers counts as red until you classify it.
3. **You agree on the gates.** Few, but strict, each written as a scenario: the bad case its check must reject, then one it must accept. Two come proposed in every project: no secret reaches the repository, and no new dependency comes in without a person seeing it.
4. **It sets up the protections and tries them.** A `CODEOWNERS` file asks for your review on what your team always approves. Then it tries four things from the environment the builder will work in, pushing to the main branch among them, and watches each one fail. If any succeeds, your blueprint says so, as `guarantee: rule`.
5. **You approve the blueprint,** with a commit or a pull request of yours. The caps of a change (rounds, minutes and lines) go in it, and you set them together.
6. **Each zone's gates get built and tried.** `gdd-checks` builds them and `gdd-review` tries to fool them on purpose. No change starts in a zone until its gates are on the main branch.

**You end up with** files you can read, which make sense without the skills:

```text
AGENTS.md                    the GDD block: the core, in under 100 words, and the map
CODEOWNERS                   what your team always approves
gdd/
├── blueprint.md             zones, colors, checks, levels and limits
├── constitution.md          the stack, the commands and the rules that hold everywhere
├── gates/<zone>.feature     the standing gates of each zone, as scenarios
├── backlog.md               what to build, each item with its expected color
├── debt.md                  known debt, with the baseline of each check
└── context/                 copies of the references a gate needs
```

## 3. Ask for a change

**You say** the change, in your own words. There's no command to remember: `gdd` loads by itself in a project that has a blueprint.

```
Add a filter by size to the catalog
```

**What happens.** The orchestrator hands out the work and builds none of it. Whoever writes the checks, whoever builds and whoever reviews each work in a separate agent, which starts knowing nothing of what the others did.

```mermaid
sequenceDiagram
    accTitle: Who does what in one change
    accDescr: You ask the orchestrator for a change. It briefs it, a second agent writes and seals the checks, a third builds, the orchestrator runs the gates, and a reviewer with a clean context gives its verdict. You approve where your blueprint asks for a person.
    actor You
    participant O as Orchestrator<br/>gdd
    participant C as Checks<br/>gdd-checks
    participant B as Builder<br/>gdd-build
    participant R as Reviewer<br/>gdd-review

    You->>O: Ask for a change
    Note over O: Brief. Its color, and what has to pass
    opt Your blueprint asks for brief-approval
        O-->>You: The scenarios
        You->>O: Approve them
    end
    O->>C: The brief
    C-->>O: The checks, sealed
    O->>B: The brief and the seal
    B-->>O: The change, on a branch of its own
    loop While a check fails, and no limit has run out
        Note over O: Gates. It runs the checks itself
        O->>B: What failed
        B-->>O: A fix
    end
    O->>R: The change, with a clean context
    R-->>O: pass, fix or stop, and the color it found
    opt Your blueprint asks for person-review
        O-->>You: The change is ready for you
        Note over You: You approve the pull request
    end
    Note over O: Ship
```

1. **Brief.** The orchestrator works out the change's color from the paths it will touch. On yellow and red it writes the brief and its scenarios: what someone can observe, with the bad cases too. When it finds a decision nobody has made, such as a limit with no number or what happens with an empty list, it asks you. Where your blueprint asks for `brief-approval`, nothing is built before you approve.
2. **Checks.** Another agent, never the one that will build, writes the checks the color asks for. It sees each one fail for the right reason, proves it can pass and seals them.
3. **Build.** The builder works on a branch of its own, with the brief and the seal. It never gets the log, or the conversation where its checks were made.
4. **Gates.** The orchestrator runs the checks itself, out of the builder's reach: CI on its branch, or a fresh clone. What the builder reports is a claim. A check that fails sends the change back with its command, its exit code and its last lines.
5. **Review.** A reviewer with a clean context runs the change and tries to make it fail. Its verdict is `pass`, `fix` or `stop`, and it works the color out again on what was actually touched. If the color rose, the change goes back to Brief.
6. **Ship.** With every check passed, the change is brought up to date with your main branch and the machine checks run again. Where your blueprint asks for `person-review`, you approve the pull request on your platform. The orchestrator never approves anything.

**What you hear.** One line per gate, passed or failed and in how many rounds. The detail comes only when something fails or you have to decide, and a summary closes it. For the filter by size it could go like this, in your agent's own wording:

```text
brief     yellow: it touches catalog/. Three scenarios wait for your approval
checks    sealed, each one seen failing first
seal      passed
tests     failed, round 1: a size with no results
tests     passed, round 2
lint      passed
review    fix: the filter is lost on page 2
review    pass, yellow
```

**You end up with** the change on your main branch and its trail next to it:

```text
gdd/briefs/<id>.md               what had to pass, on yellow and red
features/<capability>.feature    its scenarios
gdd/log/<id>.jsonl               everything that happened, one event per line, with who approved what
```

### What each color asks for

```mermaid
flowchart TB
    accTitle: What each color asks for, in the blueprint's template
    accDescr: Green asks for the seal, the tests and the linter. Yellow adds your approval of the brief before anyone builds, and a reviewer. Red adds mutation testing and your review of the change at the end.
    subgraph G["Green: the text of a button"]
        direction LR
        g1["seal"] --> g2["tests"] --> g3["lint"]
    end
    subgraph Y["Yellow: a filter by size"]
        direction LR
        y0(["brief-approval"]) --> y1["seal"] --> y2["tests"] --> y3["lint"] --> y4["review"]
    end
    subgraph R["Red: the payment gateway"]
        direction LR
        r0(["brief-approval"]) --> r1["seal"] --> r2["tests"] --> r3["lint"] --> r4["mutation"] --> r5["review"] --> r6(["person-review"])
    end
    G ~~~ Y ~~~ R
    classDef you fill:#0969da,stroke:#0969da,color:#ffffff
    class y0,r0,r6 you
    style G fill:none,stroke:#2da44e,stroke-width:2px
    style Y fill:none,stroke:#bf8700,stroke-width:2px
    style R fill:none,stroke:#cf222e,stroke-width:2px
```

The rounded blue steps are where a person approves. This is the proportion the blueprint's template brings, and yours decides: which checks each color asks for, and where a person approves, are your team's call. Whatever its color, a change also passes the standing gates of every zone it touches, and it gets its reviewer, which on green only works the color out again and checks that nothing that judges was touched.

Each color also sets the level of the agents that work on it (light, standard or deep), and your blueprint says which model each level is.

## 4. Harden what slipped through

**You say** which failure got through, and where you found it. Fixing it is a normal change, through step 3. Hardening is the other half: what it would have taken to catch it stays as a check.

```mermaid
flowchart LR
    accTitle: Hardening, step by step
    accDescr: Something slipped through. The orchestrator finds the crack in the brief and the log, proposes the scenario that would have caught it, you approve the change to the blueprint, the check is built and tried, and every later change passes it.
    S(["It slipped<br/>through"]) --> C["Find<br/>the crack"]
    C --> P["The scenario that<br/>would have caught it"]
    P --> A(["You<br/>approve it"])
    A --> K["The check is built,<br/>and tried"]
    K --> E(["Every later<br/>change passes it"])
    classDef you fill:#0969da,stroke:#0969da,color:#ffffff
    class A you
```

1. **Someone who knows that part of the product confirms it.** The failure is real, and this is where it was.
2. **The orchestrator finds the crack.** It reads the brief and the log of the change that let it through, and works out which gate should have caught it and what that gate was missing.
3. **It proposes the scenario that would have caught it,** with its bad case first, and the check that will cover it: a machine check where a machine can tell, a question for the reviewer where none can. It never proposes a paragraph of instructions, because a new line in `AGENTS.md` costs at every gate and checks nothing.
4. **You approve the change to the blueprint and its gates,** with a pull request or a commit of yours.
5. **The check gets built and tried.** `gdd-checks` builds it and `gdd-review` tries to fool it before it counts.

Hardening goes the other way too. A gate that has gone without catching anything for as long as your blueprint says gets proposed for retirement, with the same evidence, and that's yours to approve as well.

## When it stops

| You see | What it means | What you do |
|---|---|---|
| It waits for your approval | Your blueprint asks for a person there: the brief before anyone builds, or the pull request at the end | Read it and approve it. No agent can do that for you |
| `[DECISION: ...]` in a brief | A rule two people would build differently, or a limit with no number | Answer it. Your answer goes into the brief |
| A check failed | A fix round. The builder gets the command, its exit code and its last lines | Nothing, unless a limit runs out |
| `fix` from the reviewer | A fix round, with the reviewer's notes | Nothing |
| `stop` from the reviewer | The change doesn't go in, whatever else the answer says | Read why |
| The color rose | The change touched something more delicate than the brief expected, so it goes back to Brief | Approve the new brief, if that color asks for it |
| `CANNOT:` and a reason | A role stopped because the change doesn't fit as it was briefed | The brief gets a new version. It comes back to you if what has to pass changes and its color asks for your approval |
| The `seal` check failed | Something that judges the change was touched | The change stops, and the orchestrator tells you |
| A limit ran out | Too many rounds, minutes or lines for one change | Decide how to rethink it, as a series of smaller changes for instance |
| A check fails without the change too | Known debt, or an unstable check | Recording debt and quarantining a check are your team's decisions |
| You need to skip a check, in an emergency | Skipping a check loosens it | You approve it. It's logged as a waiver, with who and why, and the change goes through what it skipped afterwards |
| Your agent can't launch other agents | It gives you one line per role | Open a new session with it, such as `/gdd:gdd-build gdd/briefs/<id>.md` |

A fix never goes into a looser check. When a check looks wrong, only whoever wrote it or a person corrects it, never the agent that has to pass it.

## Reference

### The seven skills

| Skill | Who loads it | Moment | What it leaves |
|---|---|---|---|
| `gdd` | The orchestrator, on its own, when you ask for any change | All four | What comes next, and who gets launched |
| `gdd-prepare` | The orchestrator, with you | Prepare | The blueprint, the constitution and the gates of each zone |
| `gdd-brief` | The orchestrator | Brief | What has to pass, as scenarios, and the expected risk |
| `gdd-checks` | Another agent, never the one that builds | Brief, and after Prepare and Harden | The checks, each seen failing for the right reason |
| `gdd-build` | The agent that goes through the gate | Execute | The change, and the facts that prove it |
| `gdd-review` | The reviewer, with a clean context | Execute | Its verdict, and the risk worked out again |
| `gdd-harden` | The orchestrator, with you | Harden | The crack, and the check that would have caught it |

What was decided about them, and why, is in [DESIGN.md](DESIGN.md). Every file they leave in your repository, with its template, is in [CONTRACT.md](CONTRACT.md): an engine reads and writes those same files, so what you set up today still serves you later.

### What they don't guarantee

Without an engine, the agent not touching what judges it is a rule: the skills tell it so and then check that it hasn't, but nothing stops it. Your blueprint says which of three degrees you're at.

| `guarantee` | What keeps the builder away from what judges it |
|---|---|
| `rule` | The skills tell it, and the `seal` check finds out afterwards |
| `platform` | Protected branches and your platform's permissions, each one seen closing when you prepared |
| `engine` | An engine. [T-minus](https://github.com/MarsGotta/t-minus) is still in development |

### What was tried on each agent

| | Claude Code | Codex | Antigravity |
|---|---|---|---|
| Installs as a plugin | Yes | Yes | Yes |
| A role's skill stays out of the other agents | One subagent per role, which preloads its skill | It loads only when it's named, as in `$gdd:gdd-build` | No: the seven are offered to the model |
| A whole change, run by the orchestrator | Tried with a real model, in [`evals/`](../evals/) | Not tried from the plugin yet | Not tried yet |

The mapping of each role to its skill is in [adapters/](adapters/).

## Keeping them in line

`python3 scripts/check_skills.py`, from the root of this repository, checks that every skill's templates match the contract and that each skill keeps to its design. `python3 scripts/check_commands.py` runs the git commands the skills give in a throwaway repository. And [`evals/`](../evals/) holds scenarios that check, with a real model, how an agent behaves with the skills. All of it is for this repository: your project doesn't need it.
