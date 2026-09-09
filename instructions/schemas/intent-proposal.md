---
name: intent-proposal
kind: schema
path: flywheel/schemas/intent-proposal.md
version: 1
set: 1
satisfies: [109, 188]
---

# An intent proposal

A proposed intent, put to the operator from a cluster of signals or from a
gathering.

It carries the subject in one line, the signals it rests on with their weight,
what the operator would be agreeing to look at, and the elaborations proposed
to work it, each with its type.

The subject is what is unsettled, not what should be done about it. A proposal
that already contains the answer is a decision wearing an intent's clothes.

Invalid: a proposal resting on no signal, a proposed elaboration of a type the
registry does not know, a subject that duplicates an open intent without
saying how it differs, and a proposal that names a repository it has no signal
about.
