# Proposing the gates

Open this before you propose any gate. Which gates a project needs is the person's decision: what follows is where a proposal starts, never a list to impose. Start with the project minimum and the rules you propose for its red zones.

## What must never happen in this red zone

Read the sector sources as `references/research.md` describes and follow this project's data through the zone. Propose three to five losses the person would recognize. For example, a voice recorder might erase the first part when you pause, show a recording as saved when the disk was full, or overwrite yesterday's recording when you start today's. Choose what fits the zone and the product. The person can correct the rules with the blueprint.

Put each proposal in its own `Rule:` in the zone's feature. Tag the bad and good scenarios for the reviewer and also for a machine check wherever one could observe them, even before its test is written. Link the file from the zone's `gates` field so the reviewer answers each rule whenever a change touches it. In a new product the rules can be read from day one. Place their tests in the backlog as described under Gates in `gdd-prepare`. A yellow or green zone gets its own gates only when the person asks or hardening gives it one.

## What the project already has

Before you propose anything, read what already runs: the CI workflows and the checks the main branch requires, the scripts of the package manifest, the configuration of each test runner, linter and scanner, and the hooks. Run each one on the main branch, through the runner, and write down what it took and whether it passes. A check the project already has comes into the blueprint as it is, with its own command and its own numbers: replacing it is for the person to ask. What already fails goes to the baseline. Every gate runs the constitution's commands, as CI does.

## How a change goes through them

The order is the same in every project, and it is what makes the tests of a change worth anything:

1. the brief writes the change's use cases as scenarios in Gherkin, what someone can observe, with the bad cases too;
2. `gdd-checks` writes the tests of the change from those scenarios, one test for each, sees each one fail for the right reason and seals them;
3. only then is the change built, and its gates run.

"The tests of the change" in a blueprint are those. A green change has no brief and no scenarios: it goes through the checks that already exist.

## The minimum

Every project starts with these, each round and on every color, all of them together under two minutes:

- `seal`, with the template's command;
- the tests of the change, and the ones already there;
- what the stack has to catch a mistake before anything runs: a linter, and its types where it has them;
- a scan that keeps secrets out of the repository, with a mature scanner taken as it is.

Then once, before the reviewer, what takes longer, under ten minutes in all: when the project has a screen, one test that drives it through what the change touches, and only when the change touches what a person sees.

And the rules of the product in the constitution, three to seven, which every brief goes through.

## The scan for secrets, right the first time

Whichever scanner the project takes, its command:

- reports the file, the line and the kind of what it finds, never the value;
- ignores the marks in the code that exempt a line, which any change could add;
- judges what the change adds, from `<start>` to its head;
- fails when its starting point doesn't exist, instead of passing with nothing scanned;
- names its configuration and its ignore file under `gdd/`, which the seal covers, so that one a change adds elsewhere changes nothing.

An example to adapt, for gitleaks 8, a scanner in wide use and not the method's choice, with `gdd/gitleaks.toml` holding `[extend]` and `useDefault = true`:

```
git rev-parse --verify --quiet "<start>^{commit}" > /dev/null && gitleaks git --log-opts="<start>..HEAD" --config gdd/gitleaks.toml --gitleaks-ignore-path gdd --ignore-gitleaks-allow --redact --no-banner --no-color --verbose .
```

Check every flag in the scanner's own help before you write it. Then try the command once, before it goes in the constitution: in a throwaway clone in your temporary folder, commit a fake key on a line with the exemption mark and watch it fail and name the file, the line and the kind, without the value; run it again from a commit that doesn't exist and watch it fail too. So the person approves it once. The trial never touches the project: stop it when the copy can't be made or entered (`git clone -q . "$copy" || exit 1; cd "$copy" || exit 1`), and never hide what those two say. If the trial can't run where you are, leave the command as it is, write no script around it and tell the person it is untried: `standing-checks` tries it first.

## What isn't in the minimum, and why

- **Mutation testing.** It shows whether the tests catch planted bugs, and it costs minutes on every run. It comes when the tests proved weak: a reviewer found one that passed without its behavior, or a failure got past tests that ran its lines.
- **A threshold of coverage.** How many lines ran says nothing about what was checked, and a change pays to reach the number. The tests of each change already follow its scenarios.
- **Tests that drive a browser on every round.** They earn their place once the rest pass, before the review.

When hardening brings mutation testing or a threshold of coverage in, it starts at the tool's own default, looked up on the tool's own page: never a number from memory. Anything stricter comes in when hardening, with what slipped through as its evidence, and the person approves it.

## Two tiers, and what each may take

A check runs `each round` when the checks of a round still take under two minutes together with it (`seconds_each_round` in the blueprint's `limits`), and `before review` when it would take them past that: it runs once the rest pass, and all of those under ten minutes (`seconds_before_review`). Time each one on the main branch, through the runner, before you choose. A run past its tier's seconds counts as it came out, and goes to the backlog for hardening: what is slow is the check, not the change, and it is never cut off.

## What a change may take

Where a project starts, for green, yellow and red, until its own logs correct it:

| | Green | Yellow | Red |
|---|---|---|---|
| Minutes of work, without waiting for a person | 10 | 45 | 60 |
| The change's tests | none | a test per scenario, seen failing | the same, seen passing and broken once |
| Fix rounds in all | 1 | 3 | 3 |
| Runs of the checks before the review | 1 | 2 | 2 |
| The person approves the brief | no | yes, in a short message | yes |
| The review | the seal and the color | it reads the change against its brief, about five minutes, two rounds at most | it runs the change and tries to break it, two rounds at most, about fifteen minutes |
| A reading of the brief before building | no | only when the change changes data another zone owns | yes |
| Level | light | standard | standard to build, deep for its brief |

For how many open product questions to ask, follow `gdd-brief`. What the person reads to approve a brief stays under 150 words. Before briefing, propose parts if the person asks for them or for several things they could use separately. The person decides, and the first part must show them something. When a limit runs out, no new round starts, but a build already finished still goes through its gates and its review. If the person had left the decisions to the agents, nobody asks them to extend it: the change is left in the backlog, unless the next item needs the work that stopped. Every change needs the standing checks. When other work must wait, explain what one more attempt would buy and propose it now.

## The ladder

For every check, stop at the first step that works, because a deterministic check costs no tokens and always gives the same answer:

1. what the stack already brings: types, a linter rule, the test runner;
2. a mature open-source tool with a solid community or company behind it, taken as it is: no script of yours beside it for what it might miss, until something real gets past it;
3. a small script of the project's own, only when nothing exists (`gdd-checks` writes it, with its rules);
4. a new or less proven tool, only with a notice (how old it is, who maintains it, its license, what it does with the data), and the person decides;
5. a reviewer or a person, when nothing deterministic works.

A check that closes at a number takes its tool's own default, looked up on the tool's own page, or what the code as it is reaches, and never the highest, which is a gate no change gets through.
