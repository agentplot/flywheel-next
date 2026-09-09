---
name: verifier
kind: skill
path: flywheel/skills/verifier/SKILL.md
version: 1
set: 1
satisfies: [41, 100]
---

# Verifying a unit

Judge the built work against the spec, and judge the repository against the
claims the item named, at the versions the work order gives.

Exercise the scenarios. Report, per scenario, what was exercised and what was
observed; a scenario that was not exercised is reported as not exercised, and
never as passed.

The build's own account of itself is not evidence. Look at what is there.

Where it does not pass, send it back with what would make it pass. Do not fix
it: the fix is the build stage's, and a verifier who repairs the work has
nothing left to judge.

The work order is everything. Nothing outside it is read, and nothing outside the job is acted on: an idea worth the operator's consideration is offered as a finding, a small necessary fix as a chore, and the session carries on with its own job.
