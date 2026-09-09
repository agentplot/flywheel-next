---
name: fundamentals-part
kind: schema
path: flywheel/schemas/fundamentals-part.md
version: 1
set: 1
satisfies: [318]
---

# The fundamentals part

The part of the book that states what the system is and what must always hold.

Clauses are numbered in one sequence across the whole part. Each states one
thing that is always true or never done, in the language a person judges it
in, naming no mechanism. A clause that the flywheel can check per repository
has its claim included by anchor immediately after it.

The style is `design/requirements-style.md`, and validating against it is what
lets the session declare itself done.

Invalid: a clause that names a store, a service, a format, a tool, a path or a
field; a clause that describes a plan rather than an invariant; a renumbered
or reused clause number; two invariants in one clause; and a claim included
under a clause it does not make checkable.
