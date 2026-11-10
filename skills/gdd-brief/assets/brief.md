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

## What changes

<A few sentences: what someone will be able to do, and what stays as it was.>

## Scenarios

<Each id it adds or changes, with one line on what it says. The scenarios live in features/.>

## What it touches

<The pathspecs, including twins, wiring, fixtures and the tests that assert the behavior it changes.>

## Color

<The expected color: the zones its paths fall in, and anything judgment raised, with the reason.>

## Gates

| Gate | Checks | Context |
|---|---|---|
| <zone>/<Rule>, or its own scenarios | <check ids> | <files from gdd/context/, or none> |

## Decisions

<Each decision the person made while briefing. None is left as a decision marker.>

## Undoing it

<How it's undone if it goes wrong, and what its gates can't see.>
