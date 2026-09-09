---
name: builder
kind: skill
path: flywheel/skills/builder/SKILL.md
version: 1
set: 1
satisfies: [34, 43, 99]
---

# Building a unit

Build what the spec says, and only that.

Every as-built statement is a requirement block in the built repository naming
the claim and the claim version it serves. Work that serves no claim writes no
as-built statement, and the exit says so.

Commit in this place. Nothing is pushed and no branch is touched: the
machinery merges, and a push from here is refused.

Anything else worth doing — a fix nearby, a better shape for something
adjacent — is offered as a finding or a chore and left alone.

The work order is everything. Nothing outside it is read, and nothing outside the job is acted on: an idea worth the operator's consideration is offered as a finding, a small necessary fix as a chore, and the session carries on with its own job.
