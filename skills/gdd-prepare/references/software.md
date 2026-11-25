# Tests for software

Open this when the product is software, before you write the tests of a change and before you propose its gates. It says at which level each test goes, so that the tests of a change stay fast, fail for one reason and say what broke. No tool is named here: the stack's own runner decides how.

## Three levels

- **Unit.** A rule that one place of the code decides: a calculation, a validation, a state that changes. It runs in milliseconds, through the public function that decides it, with the real collaborators that live in the same process where that is feasible, and a double only for what lies outside it, such as the network, the clock or the disk.
- **Integration.** What only exists once two parts meet: something stored and read back, an endpoint and what it keeps, a component and the data it shows, the contract with another system. It takes seconds, with the project's real pieces and a local stand-in for what lies outside.
- **End to end.** The built product, used the way a person or another system uses it. It is the slowest and fails for the most reasons, so it checks that the parts are wired together, never each rule.

## Where each scenario's test goes

Each scenario's test goes in the blueprint's `tests`, which runs every round within its seconds, at the lowest level that can observe its outcome:

1. When one place of the code decides the outcome, it is a unit test, even when the scenario is told from what a person sees: the rule is what it checks, and the screen only shows it.
2. When the outcome only exists once two parts meet, it is an integration test. So is a scenario that creates, uses up or ends something, or changes a value in another zone: its test reads the result where the scenario says.
3. When only the built product seems to show it, look for the part below that decides what it shows, and test that. If there is none, say so under `MISSING:`: a test of the whole product doesn't fit in a round.

## The end-to-end test of a change

When the change touches what a person sees, the blueprint's `browser` check, where it has one, drives the built product through what the change touches, once, before the review. It follows the scenario that crosses most of those parts, cites it as `gdd:<id>`, and checks the wiring with one value the part below gives, enough to reject a screen that shows a constant. The other cases of that rule stay in the tests below. A second one comes only when hardening brings it, with what slipped through.

## The shape of the whole

Many tests at the bottom, fewer in the middle and a few at the top. Where the interface is built of components, a component rendered with its data holds most of what a screen shows, and the middle grows. The project's own mix wins when its team has one, and hardening corrects it with what slipped through, never with a count of tests.

The shape to avoid is the inverted one: many end-to-end tests and few below them. They are slow, they fail at random, a failure doesn't say which rule broke, and on red each one has to be seen passing on a throwaway version of the whole product, where a unit test needs one of a single function.

## Signs a test sits at the wrong level

- An end-to-end test that checks a number, a message or a rule: that part is a unit test, and the end-to-end one keeps only the wiring.
- A unit test that replaces everything around the code with doubles and checks only that they were called: it proves the doubles. It goes up a level, with the real pieces.
- The checks of a round run past their seconds in the blueprint's `limits`: look first for tests above the level they need.
- A test that drives a browser fails now and then: find what it waits for before anything else.
