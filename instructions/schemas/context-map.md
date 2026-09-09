---
name: context-map
kind: schema
path: flywheel/schemas/context-map.md
version: 1
set: 1
satisfies: [198, 211]
---

# The system context map

The map of the bounded contexts the blueprints describe, in the terms of
domain-driven design.

An entry is a context, the elements it names, and the relationships to other
contexts, each typed by its pattern with an upstream and a downstream side.
Every element carries its home — the repository the thing lives in — and the
claims attached to it.

Every edit is made through the map tool, so the commit carries the trailer the
blueprints' hook requires. A hand edit of the map files is refused, because
the rendered map and the source would then disagree with nothing to say which
is right.

Invalid: an element with no home, a relationship with no pattern, a
relationship whose two sides are the same context, a claim attached to an
element that does not exist, and an edit committed without the tool's trailer.
