---
name: units/default/apply
kind: schema
path: flywheel/schemas/units/default/apply.md
version: 1
set: 1
satisfies: [37, 99]
---

# default, the build step

`opsx apply` builds what the spec says and nothing else.

Every as-built statement is a requirement block in the built repository naming
the claim and the claim version it serves. Work that serves no claim writes
no as-built statement and says so in the exit.

Commits are the deliverable; the merge is the machinery's. Nothing is pushed.

Invalid: a build that changes the spec to match the code, an as-built
statement naming a claim the item does not cite, work outside the item's
scope carried in as a drive-by, and a commit that leaves the tree failing its
own tests.
