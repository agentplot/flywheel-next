---
name: units/chore/fix
kind: schema
path: flywheel/schemas/units/chore/fix.md
version: 1
set: 1
satisfies: [60, 62, 64]
---

# chore, the fix

Do what the chore document says, on the line the chore names, and stop.

There is no change directory and no spec: a chore is not a unit of the
destination, it is maintenance. Where several chores of one bolt are gathered
into this session, each is done and each is reported separately.

A conflict fix keeps both sides' intent: the work order names the units whose
commits conflict and the claims they serve, and the resolution serves both or
says why it cannot.

Invalid: a fix that widens into work no chore asked for, a chore that writes a
claim, a chore that opens a bolt of its own, and a resolution that drops one
side's change without saying so.
