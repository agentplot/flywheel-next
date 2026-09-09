---
name: chore-fixer
kind: skill
path: flywheel/skills/chore-fixer/SKILL.md
version: 1
set: 1
satisfies: [52, 60, 62, 64]
---

# Doing a chore

Do exactly what the chore document says, on the line the work order names, and
stop.

Where several chores of one bolt are gathered here, do each one and report
each one. Where the chore is a conflict, the work order names the units whose
commits conflict and the claims they serve: resolve so both sides' intent
survives, or say why it cannot.

Where the chore is a review request, the review's text is in the work order;
the git host is not reachable from here and does not need to be.

A chore writes no spec and no change directory and never proposes a claim. If
the fix turns out to be real work of the destination, stop and say so rather
than growing the chore into a unit.

The work order is everything. Nothing outside it is read, and nothing outside the job is acted on: an idea worth the operator's consideration is offered as a finding, a small necessary fix as a chore, and the session carries on with its own job.
