# Proposing the gates

Open this before you propose any gate. Which gates a project needs is the person's decision: what follows is where a proposal starts, never a list to impose. A project starts with the minimum, and its gates grow only by hardening, when something slipped through.

## What the project already has

Before you propose anything, read what already runs: the CI workflows and the checks the main branch requires, the scripts of the package manifest, the configuration of each test runner, linter and scanner, and the hooks. Run each one on the main branch, through the runner, and write down what it took and whether it passes. A check the project already has comes into the blueprint as it is, with its own command and its own numbers: replacing it is for the person to ask. What already fails goes to the baseline. Every gate runs the constitution's commands, as CI does: in the T-minus pilot, a gate passed while the project's linter failed, because it didn't run the same lint.

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
- a scan that keeps secrets out of the repository, with a mature scanner taken as it is. In one run, each zone got a scan written by hand while a mature one was installed, every review found one more secret it missed, and no change started.

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

Check every flag in the scanner's own help before you write it. Then try the command once, before it goes in the constitution: in a throwaway clone, commit a fake key on a line with the exemption mark and watch it fail and name the file, the line and the kind, without the value; run it again from a commit that doesn't exist and watch it fail too. So the person approves it once: in one run, the command was fixed twice after the approval, with an approval each time, first because a key with the exemption mark got through, then because it didn't say the file, the line and the kind. The trial never touches the project: stop it when the copy can't be made or entered (`git clone -q . "$copy" || exit 1; cd "$copy" || exit 1`), and never hide what those two say. In one run the clone failed without a word, the trial's commits landed in the project itself, and the hard reset that took them out also took files written a moment before.

## What isn't in the minimum, and why

- **Mutation testing.** It shows whether the tests catch planted bugs, and it costs minutes on every run. It comes when the tests proved weak: a reviewer found one that passed without its behavior, or a failure got past tests that ran its lines. In one run, a change passed all its tests at the first try, and four more rounds went into raising its mutation score to the highest, the number Prepare had set.
- **A threshold of coverage.** How many lines ran says nothing about what was checked, and a change pays to reach the number. The tests of each change already follow its scenarios.
- **Tests that drive a browser on every round.** They earn their place once the rest pass, before the review: in one run, a change went through gates of about seven minutes, round after round.

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
| Questions to the person | none | one | two |
| The person approves the brief | no | yes, in a short message | yes |
| The review | the seal and the color | it reads the change against its brief, about five minutes, two rounds at most | it runs the change and tries to break it, two rounds at most, about fifteen minutes |
| A reading of the brief before building | no | only when the change changes data another zone owns | yes |
| Level | light | standard | standard to build, deep for its brief |

What the person reads to approve a brief stays under 150 words. A request is split only when it would go over the blueprint's lines or cross two red zones, and its first part shows the person something. When a limit runs out, no new round starts, but a build already finished still goes through its gates and its review: in one run, a change stayed built and never went in because its rounds ran out before its gates. If the person had left the decisions to the agents, nobody asks them to extend it: the change is split or left in the backlog, unless every change waits on what stopped, such as the standing checks, and then the person is asked at once for one more round.

## The ladder

For every check, stop at the first step that works, because a deterministic check costs no tokens and always gives the same answer:

1. what the stack already brings: types, a linter rule, the test runner;
2. a mature open-source tool with a solid community or company behind it, taken as it is: no script of yours beside it for what it might miss, until something real gets past it;
3. a small script of the project's own, only when nothing exists (`gdd-checks` writes it, with its rules);
4. a new or less proven tool, only with a notice (how old it is, who maintains it, its license, what it does with the data), and the person decides;
5. a reviewer or a person, when nothing deterministic works.

A check that closes at a number takes its tool's own default, looked up on the tool's own page, or what the code as it is reaches, and never the highest, which is a gate no change gets through.
