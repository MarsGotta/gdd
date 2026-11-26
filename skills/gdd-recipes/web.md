# Gates for the web

Open this when the project builds for the web. It has four parts: what usually goes wrong, what to start with, what to add when something slips through, and the traps of setting it up. No tool is named here: look them up for the stack, from each tool's own page, as `../gdd-prepare/references/research.md` says.

A throwaway draft, before the blueprint, is a static page that opens in a browser, with made-up data and no server.

## What usually goes wrong

Candidates for the rules of the product in the constitution, for the person to correct in their own words. Take only those the product has.

- **Money.** Check that amounts agree across steps and use the required precision. Look for repeated effects, reversals larger than the original, quantities below zero, use beyond what is available and changes to a final record. Submitting the same shared expense twice must never double what anyone owes for it.
- **Accounts and personal data.** One account seeing another person's private records, a password stored so it can be read, a session still open after logging out, personal data in a log or an error. What one account can see, another can't.
- **Data loaded from another system.** A record loaded twice or lost halfway, a load that half fails and reports success, a format that changes without warning. A load lands whole, or not at all.
- **The interface.** A page that misstates the current state, a form that loses what was typed on an error, what works only with a mouse or at one width. What a page shows is what the data says.

## What to start with

The minimum of `../gdd-prepare/references/gates.md`, and:

1. tests of those rules, written from the scenarios of each change, each round, each at the level `software.md`, next to this recipe, gives it, never through the browser;
2. one test that drives a browser through what the change touches, once before the review and only when the change touches what a person sees: the checks before the review take under ten minutes in all;
3. nothing else until something slips through.

## First times that usually need a gate

On the web, besides software's: the first form that sends something, the first page behind a sign-in, the first page that takes a payment, and the first cookie, tracker or third-party script on a page. A change that brings one when no gate covers it waits for that gate, as `../gdd-brief/references/new-risk.md` says.

## What to add when something slips through

- A check of accessibility on the pages a change touches, when a page shipped that someone couldn't use.
- Mutation testing on the unit tests of the files a change touches, when a test passed without its behavior.
- The browser tests on more engines, when something worked in one and not in another.
- A scan of the dependencies, when one with a known hole got in.

## The traps of setting it up

- **Each runner, its own files.** Unit tests and browser tests in separate folders, and each runner given a pattern that takes its folder and leaves out the other's.
- **Mutation runs the unit tests.** A mutation tool drives a unit runner and doesn't find browser tests, so the page a browser test checks comes out with every mutant alive: 0 % on a cart that worked. It mutates only the files of rules, numbers and messages, which have unit tests, and leaves out the page that wires them. A rule a scenario asks for that only a browser test checks is a test at the wrong level, as `software.md` says.
- **A port and a build folder for each copy.** Builders and reviewers work in copies side by side, so each takes its port and its build folder from an environment variable, never a fixed one. The constitution's `Use it` command takes them the same way.
- **Nothing reloads during a test.** A development server reloads when a file changes, and a test that runs while one is being written fails at random. Browser tests run against the built product, or a server with reloading off.
- **The test makes what it needs.** A temporary folder, a database, a server: its setup creates them, outside the repository, and removes them.
- **Options in the runner's own configuration,** not in the environment of the language, each checked against the version the constitution names before it is written down.
- **A test waits for what it watches,** a text on the screen or a response, never for a fixed time, and keeps its tool's default timeout: a test that needs more has a cause to find.
