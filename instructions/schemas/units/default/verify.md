---
name: units/default/verify
kind: schema
path: flywheel/schemas/units/default/verify.md
version: 1
set: 1
satisfies: [41, 100]
---

# default, the verify step

`opsx verify` judges the built work against the spec, and the verdict judges
the repository against the claims the item named, at the versions the work
order gives.

The report says, per scenario, what was exercised and what was observed. A
scenario that was not exercised is reported as not exercised, never as passed.

The judgment is the verifier's own. Agreement with the builder is not
evidence.

Invalid: a report that cites the build's own account as its evidence, a
verdict on a claim the item did not name, a pass over a scenario no one ran,
and a send-back with no statement of what would make it pass.
