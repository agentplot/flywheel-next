---
name: claim
kind: schema
path: flywheel/schemas/claim.md
version: 1
set: 1
satisfies: [97, 100, 223]
---

# A claim

A requirement block in the blueprints' OpenSpec specifications, in the
intent's delta until the intent lands.

It carries a stable name that says what is true and not what was done, one
statement of what holds at the destination, and at least one scenario saying
how one would know it holds. The version is the hash of the block's text, so
it moves when the text moves and at no other time; it is never written by
hand.

The scope says which repositories the claim is judged in. A claim in scope
nowhere is a claim no one will ever verify.

Invalid: a block with no scenario, a name that describes work rather than a
state of the world, two statements in one block, a claim whose scenario could
be true while the statement is false, and a claim written by the session that
built the thing it judges (34a).
