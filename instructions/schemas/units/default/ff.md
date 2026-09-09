---
name: units/default/ff
kind: schema
path: flywheel/schemas/units/default/ff.md
version: 1
set: 1
satisfies: [37, 190]
---

# default, the spec step

`opsx ff` writes the change directory and the spec from the work item and
from nothing else.

The spec says what the built repository will be true of when the work lands,
as requirement blocks with scenarios, each naming the standing claim it
serves. It restates no reasoning: the reasoning is in the chapters copied into
the place, and repeating it here makes two sources that will disagree.

Invalid: a spec that names a claim not among the item's, a spec written from
the code rather than from the item, a change directory with no delta, and a
scenario that cannot fail.
