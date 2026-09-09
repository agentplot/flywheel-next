---
name: diagram-logical
kind: schema
path: flywheel/schemas/diagram-logical.md
version: 1
set: 1
satisfies: [190]
---

# A logical diagram

One picture of how the thing is arranged: the components, what each is
responsible for, and what crosses between them.

The same house style as the conceptual diagram — the same palette, the same
shapes, the same direction of flow — so that the two read as one pair. What
differs is the altitude: this one may name a store, a service or a boundary,
and it still names no file and no function.

Every crossing is labelled with what crosses, not with the mechanism that
carries it.

Invalid: a diagram that contradicts the chapter it sits in, a component with
no responsibility stated, a crossing that appears in no claim and no chapter,
and a picture drawn by hand where the model can derive one.
