---
name: units/fast/apply
kind: schema
path: flywheel/schemas/units/fast/apply.md
version: 1
set: 1
satisfies: [37, 99, 100]
---

# fast, the build

Build what the spec written moments ago says, and write the as-built
statements naming the claims served.

There is no verify stage. The work is small enough that a separate judgment
would cost more than it is worth, and the claim's cell is judged by planning's
next run instead. That is the bargain of this type: no verdict is produced
here, and none is faked.

Invalid: an exit claiming a verdict, a build that outgrew the type and should
have been sent back as a default unit, and an as-built statement with no
matching scenario.
