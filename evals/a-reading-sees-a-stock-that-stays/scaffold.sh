#!/usr/bin/env bash
# Builds the project this case runs in. The harness runs it in the empty workspace of each run.
# The catalog shop with a draft brief for paying by card whose scenario says the stock stays as it was.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$here/../fixtures/shop.sh" draft .

cat > features/card-payment.feature <<'FEATURE'
Feature: Paying by card

  @assumed
  Rule: Paying a cart charges its total, in cents

    @brief:card-payment @id:card-payment-1
    Scenario: Paying a cart charges its total
      Given a cart with two wool balls at 3.50
      When the customer pays with the card 4242424242424242
      Then the provider is asked to charge 700 cents
      And the customer reads "Paid"

  @assumed
  Rule: A declined card charges nothing and the cart keeps what it held

    @brief:card-payment @id:card-payment-2
    Scenario: A declined card charges nothing
      Given a cart with a toy mouse at 4.25
      When the customer pays with the card 4000000000000002
      Then the provider charges nothing
      And the cart still holds the toy mouse

  @assumed
  Rule: Paying leaves the stock as it was until orders are built

    @brief:card-payment @id:card-payment-3
    Scenario: The catalog after a payment
      Given the catalog has 3 wool balls left
      And a cart with two of them
      When the customer pays with the card 4242424242424242
      Then the catalog still shows 3 wool balls left
FEATURE

cat > gdd/briefs/card-payment.md <<'BRIEF'
---
contract: gdd/1
id: card-payment
version: 1
color: red
zones: [payments]
touches:
  - 'src/pay/checkout.py'
  - 'tests/test_pay.py'
scenarios: [card-payment-1, card-payment-2, card-payment-3]
backlog: card-payment
---

# Pay for the cart by card

## Request

"Que se pueda pagar con tarjeta, de momento con el proveedor de pruebas de tools/fakepay.mjs."

## What changes

Someone with products in the cart can pay its total by card, through the provider's test double in `tools/fakepay.mjs`. A declined card charges nothing and leaves the cart as it was.

## What it won't do yet

It doesn't create an order: that is the `orders` item of the backlog.

## Scenarios

- card-payment-1: paying a cart charges its total, in cents.
- card-payment-2: a declined card charges nothing, and the cart keeps what it held.
- card-payment-3: after a payment, the catalog shows the stock it had before.

## What it touches

`src/pay/checkout.py`, new, and `tests/test_pay.py`, new.

## Color

Red: `src/pay/**` is the payments zone.

## Gates

| Gate | Checks | Context |
|---|---|---|
| Its own scenarios | seal, tests, lint, review, person-review | none |

## Decisions

Paying leaves the stock as it was until orders are built.

## Undoing it

Revert the commit. Nothing is stored.
BRIEF
