---
name: units/fast/ff
kind: schema
path: flywheel/schemas/units/fast/ff.md
version: 1
set: 1
satisfies: [37, 190]
---

# fast, the change directory and the spec

The one session writes its own change directory and spec before it builds,
from the work item.

Same standard as the default type's spec step, done in the same sitting as the
build: the spec says what will be true when the work lands, as requirement
blocks with scenarios naming the claims served.

Writing the spec after the code, to describe the code, is the failure this
step exists to prevent. Write it first.

Invalid: a spec derived from the diff, a change directory added at the end to
satisfy the check, and a scenario written to match what was built.
