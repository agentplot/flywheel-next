---
name: proposal
kind: schema
path: flywheel/schemas/proposal.md
version: 1
set: 1
satisfies: [28, 172, 190]
---

# A planning proposal

One document per planning run, put to the operator as one thing.

It carries the case for the batch — what in the backlog it closes and why now
— and then the units. Each unit names its type, the standing claims in scope
it serves (or states plainly that none fits), what it depends on, and a size
estimate in slot-days, calibrated against the actuals of the same type in this
repository.

A unit is a piece of work with one outcome, not a heading over several. If a
unit's description needs the word "and" to hold together, it is two units.

Invalid: a unit citing a proposed claim, a unit proposing a claim of its own, a
size estimate with no basis in the actuals, a unit that depends on a unit not
in the batch and not already landed, and a proposal that leaves out a stale
cell it was charged with without saying why.
