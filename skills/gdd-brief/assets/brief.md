---
contract: gdd/1
id: <id>
version: 1
color: <expected color>
zones: [<zone ids>]
touches:
  - '<pathspec>'
scenarios: [<ids of the scenarios it adds or changes>]
backlog: <item id>
---

# <What the person gets, in a few words>

## Request

<The person's words.>

## Closing it

Expected: <what changes for whoever uses it, in the person's words>
Signal: <what will show it, from where the builder can't write>
Where: <where it will be looked at>
<Or, when the change promises nothing to whoever uses the product, as the blueprint's Closing says, only this line: Nothing to close: <why>>

## Rules

<Link each features/<capability>.feature file that holds the rules for this change.>

## What it won't do yet

<Up to three things the person might expect from the request and won't get, each as a consequence they can weigh, with the backlog item that will do it, or none.>

## Undoing it

<In one line, how it's undone if it goes wrong and what its gates can't see.>

## What it touches

<The pathspecs, including twins, wiring, fixtures and the tests that assert the behavior it changes.>

## Color

<The expected color: the zones its paths fall in, and anything judgment raised, with the reason.>

## Gates

| Gate | Checks | Context |
|---|---|---|
| <zone>/<Rule>, or its own scenarios | <check ids> | <files from gdd/context/, or none> |

## Decisions

<The technical decisions you took.>
