---
summary: The Decision Inbox of a change: what waits for you.
runs: you
---

## Who runs it

You, when a change needs you or has parked.

## Examples

Your agents write to it with [`gdd note`](note.md); gdd also keeps it in
`.gdd/changes/<id>/inbox.md`.

```bash
gdd inbox cart-total
```

You should see:

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
# Decision Inbox · cart-total · Show the cart total

Color ? · lane ? · state open

## Needs your decision
- Should the total include taxes? I recommend no, taxes come later

## Decided without you
- Totals round to two decimals

## Parked
- Nothing.
```

## See also

- [Unblock a parked change](../../how-to/parked-change.md): what to do with a parked inbox.
