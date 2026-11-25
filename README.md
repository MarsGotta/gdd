# Gate-Driven Development

[![Checks](https://github.com/MarsGotta/gdd/actions/workflows/checks.yml/badge.svg)](https://github.com/MarsGotta/gdd/actions/workflows/checks.yml)
[![Version](https://img.shields.io/github/v/tag/MarsGotta/gdd?sort=semver&label=version)](https://github.com/MarsGotta/gdd/tags)
[![License](https://img.shields.io/badge/license-MIT%20and%20CC%20BY%204.0-blue)](#license)

**Add gates to your project and automate with AI agents without losing quality.**

Your agent says "done." The gate checks if it's true.

**[Documentation](https://gate-driven.dev/en/docs/)** · **[Manifesto](https://gate-driven.dev/en/manifesto/)** · **[Guide to the skills](skills/README.md)** · **[En español](https://gate-driven.dev/documentacion/)**

You ask your agent for something, it works for a while and tells you it's done. You try it, and it isn't. GDD is the method I use so that the agent's word doesn't open anything: before it starts, you decide what it has to prove, and checks it can't touch decide whether it did.

```mermaid
flowchart LR
    accTitle: A change, with a gate in the way
    accDescr: You ask for a change. What it has to prove is decided before anyone builds. Your agent builds it, and the gate runs checks the agent can't touch. If they pass, the change reaches your main branch. If one fails, it goes back to the agent.
    Y(["You ask<br/>for a change"]) --> B["What it has to prove<br/>is decided first"]
    B --> A["Your agent<br/>builds it"]
    A --> G{"The gate"}
    G -- "its checks pass" --> M(["Your main branch"])
    G -- "a check fails" --> A
    %% An empty node keeps the right side free: GitHub draws its zoom buttons there
    M ~~~ Z["zoom buttons"]
    classDef you fill:#0969da,stroke:#0969da,color:#ffffff
    classDef room fill:none,stroke:none,color:transparent
    class Y you
    class Z room
```

## Why GDD

Agents write fast and almost right. If you review everything, you become the slowest step. If you review nothing, the errors reach production. And if the agent can touch what judges it, it learns to approve itself.

- **What has to pass is written before anyone builds.** Your agent writes the rules of the change in plain words, with a few scenarios under each one, and where your blueprint asks for it you approve them before there is any code.
- **Whoever builds doesn't judge.** Another agent turns each scenario into a test and seals it. The builder works without that conversation, and a reviewer that heard none of it reads the change against what has to pass, and on red also runs it and tries to make it fail.
- **You're asked where the risk is.** When you set GDD up, what is delicate in your project gets a color, green, yellow or red, and the color of what a change touches decides how much of it reaches you.
- **The bookkeeping isn't in any agent's memory.** A small command, the layer, keeps the order of every change, counts its limits, writes its log and refuses what comes out of order. Your CI, once you set it up, checks every log where no agent writes.
- **It runs on what your project already has.** Your tests, your linter, your CI and git. There's nothing to sign up for.
- **Gates grow and shrink.** What slips through becomes one more check, and a gate that stops adding anything gets relaxed or retired.

## Quick start

1. [Install the plugin](#install) for your agent.
2. Open your agent in your project's folder, on its deepest model, and say:

   ```
   Set GDD up in this project
   ```

   If the project is new, add what you're going to build. Your agent proposes what is delicate in your project (the zones, each with its color), what every change has to prove (the gates) and which model does each kind of work, in `gdd/blueprint.md`. Read it, you or your team, and say yes or what to change. Then it sets up the checks your blueprint names and tries to beat each one on purpose. Nothing gets built before that.

3. Ask for changes in your own words:

   ```
   Add a filter by size to the catalog
   ```

4. When something gets past every gate, say what it was:

   ```
   The size filter is lost on page 2, and no check caught it
   ```

   Fixing it is a change like any other. Your agent also finds which gate should have caught it and proposes the check that would have. Once you approve it, every change in that zone passes it.

You never call a skill by name: in a project with a blueprint, your agent loads `gdd` on its own when you ask for a change. Where it needs you it asks one thing at a time, with what it would do, so that a yes is enough.

## Install

**What you need:**

- **A git repository,** with git 2.38 or later.
- **An agent that loads skills:** Claude Code, Codex or Antigravity.
- **[Rust](https://rustup.rs) 1.89 or later,** to build the layer, `gdd`: once in each copy of your project, and in your CI. The first build downloads what it depends on from crates.io, so it needs the network that once.
- **macOS or Linux.** Nobody has tried it on Windows yet.

The skills install as a plugin, with a command or two.

**Claude Code** (2.1.275 or later), inside a session:

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

Then check it's there, and come back to the same table when you want a newer version:

| Agent | Check | Update |
|---|---|---|
| Claude Code | `claude plugin list` shows `gdd@gdd`, enabled | `claude plugin marketplace update gdd`, then `claude plugin update gdd@gdd` |
| Codex | `codex plugin list` shows `gdd@gdd`, installed and enabled | `codex plugin marketplace upgrade gdd`, then `codex plugin add gdd@gdd` |
| Antigravity | `agy plugin list` shows `gdd`, enabled, with 8 skills | `git pull` in your clone, then `agy plugin install gdd` again |

If you'd rather keep the skills inside your project, [copy them by hand](skills/README.md#1-install-the-skills).

**The layer.** The plugin also brings `gdd`, a small command that keeps the order of every change, counts its limits and writes its log, so your agent doesn't keep that bookkeeping from memory. It comes as source, in the plugin's `engine/` folder, and the first time your project needs it your agent builds it and leaves it at `.gdd/gdd`, which git ignores. If you don't have Rust, your agent tells you in one line and waits. Your CI builds the same command, to check every change's log out of the agents' reach.

## What a change looks like

Every change gets the color of the most delicate thing it touches, and the color decides what you do. Where a person approves is your decision when you set GDD up, and this is how the skills propose it:

| The change touches | For example | What you do |
|---|---|---|
| Something green | The text of a button | Nothing. You hear about it when it's in |
| Something yellow | A filter in the catalog | You approve what it has to do, in a short message, before anyone builds |
| Something red | Anything that moves money | You approve what it has to do, and you review the change at the end |

**What you approve.** On yellow and red, before anyone builds, your agent sends you the rules of the change in under 200 words, or 250 on red, unless you ask for more detail, in whole sentences: each new or changed rule, its own assumptions marked as its own, what it took as true without checking, how you'll know it helped, and the file where you can read the cases. For the size filter it could say:

> You'll see only the products that come in the size you pick.
>
> I assumed sizes are listed from small to large, not in alphabetical order.
>
> We'll know it helped if fewer people leave the catalog without opening a product, in the shop's visit records.
>
> You can read the cases in `features/catalog.feature`. Shall I go ahead with these rules?

Say yes, or correct a rule. The file holds the same rules, each with the scenarios that will become its tests:

```gherkin
Feature: Catalog

  Rule: Picking a size shows only the products that come in it

    @brief:size-filter @id:catalog-4
    Scenario: A jacket only in L is hidden when you pick M
      Given a T-shirt in sizes S and M, and a jacket only in L
      When the customer picks size M
      Then the T-shirt is listed and the jacket is not

  @assumed
  Rule: Sizes are listed from small to large

    @brief:size-filter @id:catalog-5
    Scenario: The size list follows the size chart
      Given products in sizes L, S and M
      When the customer opens the size filter
      Then the sizes read S, M, L
```

Each scenario sits under its rule, has an `@id` and takes one action, and the layer refuses a brief whose new scenarios don't.

**What you hear.** When the change is in, your agent tells you in two or three sentences what changed in your product. The detail of a check only comes if it failed or you ask. For the size filter it could say:

> The size filter is in: you'll see only the products that come in the size you pick. Every check passed. The first version still listed the jacket under M, and the tests try that case now.

**What waits.** Once it's in use, a change that promised you something waits in sight, however long, until you say whether it helped: yes, not yet or no. It holds nothing up, and your agent only mentions it in a line. When you ask, it looks where the brief said and brings you what it sees, without deciding for you.

**What stays.** The whole trail stays in your repository, next to your code:

```text
gdd/blueprint.md                 your zones, their colors and their gates, as you approved them
features/<capability>.feature    the rules of your product and their scenarios
gdd/briefs/<id>.md               what had to pass for each change, on yellow and red
gdd/log/<id>.jsonl               everything that happened, one event per line, with who approved what
```

**When it needs you.** Besides an approval, your agent comes to you with a decision nobody has made, or with something that stopped the change, such as a check that keeps failing or a limit that ran out. Answer, or tell it to decide: it then decides how the work gets done, and any rule of your product it takes from what is usual in your sector, it tells you as an assumption. What loosens a check or can't be undone stays with you. The guide has [every case](skills/README.md#when-it-stops).

Three commands read the trail for you, from your project's folder. `.gdd/gdd next` says what comes next in your project, including the changes in use that wait for your verdict. `.gdd/gdd waiting` lists every question waiting for you, oldest first. `.gdd/gdd metrics --all --summary` says how your changes went: how they ended, how long each took without the waits for you, how often they got through the gates and the reviewer the first time, how many that shipped were reverted later, and what each check and each agent cost.

## The five moments, and what holds each one up

Underneath, every change goes through the method's five moments. What carries each one is written down, so it doesn't rest on your agent remembering what to do: a skill it loads for that moment, the files it leaves in your repository, and the layer, which refuses what comes out of order.

| Moment | What it's for | What holds it up |
|---|---|---|
| **1. Prepare.** At the start, and again when a gate is missing, isn't needed or is badly defined | It has two parts. First the blueprint: what can never happen in your product, what is delicate, which gates you need and where a person approves. Then the gates it defined get built. You build them, or your team does, with the agents and tools you use, and you try to beat them on purpose. No change gets in until they hold. | `gdd-prepare` writes the blueprint and the constitution with you. `gdd-checks` sets up the standing checks and `gdd-review` tries them. It all stays in `gdd/blueprint.md`, `gdd/constitution.md` and `gdd/gates/`. |
| **2. Brief.** On every change, before it starts | What has to be true gets written down before anyone builds: the rules the change touches, a few scenarios and a first estimate of its risk. | `gdd-brief` writes the rules, the scenarios and the color, in `features/` and `gdd/briefs/`. On red, `gdd-review` reads the brief before you approve it. `gdd-checks` writes a test for each scenario, sees it fail and seals it. The layer refuses a scenario with no rule above it, and it lets nobody build before your yes, where your blueprint asks for one. |
| **3. Execute.** On every change | An agent builds the change and gets through the gates without touching what judges it. A reviewer that heard none of it tries to make it fail, and the risk is worked out again from what was touched. | `gdd-build` builds. The layer runs the gates from the seal, counts the limits and writes the log in `gdd/log/`. `gdd-review` reviews with a clean context. Your CI, once you set it up, checks every log and runs the gates again where no agent writes. |
| **4. Close.** Once the change is in use | You look at whether it helped, against what its brief expected before anyone built it. The verdict is yours, or belongs to whoever your blueprint names: yes, not yet or no. | `gdd-brief` writes in the brief what is expected, the signal that will show it and where to look. The layer keeps what is in use in sight, with how long it has waited, and holds nothing up. When you ask, `gdd-close` gathers what can be seen there, proposes no verdict, and the layer records yours in `gdd/log/`. |
| **5. Harden.** When something slips through or repeats, and when a gate stops adding anything | What slipped through becomes one more check. What no longer adds anything gets relaxed or retired. Both change the blueprint, so you approve them, or your team does. | `gdd-harden` finds which gate should have caught it and proposes the check, or reads the logs for what a gate caught and what it cost. `gdd-checks` builds the check. |

Under all five there are two things more. `gdd`, the skill your agent loads on its own when you ask for a change, knows what comes next and who gets launched. And the layer, `.gdd/gdd`, keeps the order, the limits and the log, so that none of it depends on an agent's memory.

How far that holds depends on where your gates run. With the skills alone, the agent not touching what judges it is a rule it is told and then checked on. With protected branches and your CI, your platform enforces it. The guide says [what the skills don't guarantee](skills/README.md#what-they-dont-guarantee).

## Status

- **Version 0.21, with contract `gdd/1`.** The contract lists every file the skills leave in your repository, with its format. An engine will read and write those same files, so what you set up today will still serve you later.
- **Each of the five moments has a skill.** Close waits for your verdict once a change is in use, and holds nothing up.
- **The engine that enforces the method,** [T-minus](https://github.com/MarsGotta/t-minus), has its own repository and is still in development.

## Documentation

| If you want to | Read |
|---|---|
| Understand the idea, told as an escape room | [The idea](https://gate-driven.dev/en/docs/the-idea/) |
| Follow four changes to an online shop, with nothing to install | [Hands on](https://gate-driven.dev/en/docs/hands-on/) |
| Start with the skills: install, prepare, first change | [Get started](https://gate-driven.dev/en/docs/get-started/) |
| Put up a gate by hand: CI, tests, `CODEOWNERS` | [A gate by hand](https://gate-driven.dev/en/docs/start-today/) |
| See when each thing happens | [The five moments](https://gate-driven.dev/en/docs/the-moments/) |
| Learn how a gate gets fooled, and what stops it | [How a gate gets fooled](https://gate-driven.dev/en/docs/how-a-gate-gets-fooled/) |
| Compare it with TDD | [What about TDD?](https://gate-driven.dev/en/docs/and-tdd/) |
| Find out what to do when a change stops | [When it stops](https://gate-driven.dev/en/docs/when-it-stops/) |
| See what changed in each version, and how to update | [What's new](https://gate-driven.dev/en/docs/whats-new/) |
| Go through every step of the skills, and what happens inside | [The guide to the skills](skills/README.md) |
| Look up the files the skills leave, and their templates | [CONTRACT.md](skills/CONTRACT.md) |
| Know why the skills are the way they are | [DESIGN.md](skills/DESIGN.md) |
| Read the reference for everything else | [The Manifesto](https://gate-driven.dev/en/manifesto/), version 0.12 |

## What's in this repository

- [`manifesto/`](manifesto/): the Manifesto, version 0.12, in English and Spanish. If anything else contradicts it, the Manifesto wins.
- [`web/`](web/): the source of [gate-driven.dev](https://gate-driven.dev), where the whole method is explained in both languages.
- [`skills/`](skills/): skills for your coding agent, so you can apply GDD in your own project without an engine. They install as a plugin on Claude Code, Codex and Antigravity.
- [`engine/`](engine/): the layer, `gdd`, which the skills need from version 0.12: it keeps the order, the accounting and the evidence of every change, and the agents keep the judgment. It isn't an engine in the method's sense: while an agent runs it, it's within the agent's reach, and what keeps it honest is your CI.
- [`evals/`](evals/): scenarios that check, with a real model, how an agent behaves with those skills.

## License

GDD is free to use, by anyone and for anything. The Manifesto and the website's texts are under [CC BY 4.0](LICENSE-docs.md). The skills, the scripts and the website's code are under the [MIT license](LICENSE).

## How to cite

If you write about GDD or build on it, credit it like this:

> Marcela Gotta. *Gate-Driven Development*. 2026. https://gate-driven.dev

GitHub's "Cite this repository" button gives the same in other formats, from [`CITATION.cff`](CITATION.cff).
