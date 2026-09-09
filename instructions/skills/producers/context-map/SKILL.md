---
name: producers/context-map
kind: skill
path: flywheel/skills/producers/context-map/SKILL.md
version: 1
set: 1
satisfies: [198, 211]
---

# Producing the context map

Move the map whenever a claim is written or amended, in the same commit.

Edit through the map tool so the commit carries its trailer. Place a new
element in the context that owns it, give it a home, and attach the claims
that constrain it.

A relationship is typed by its pattern with an upstream and a downstream side.
If the pattern is not obvious, the coupling is probably not understood yet;
that is worth a finding.

Done when `map-check` passes and `flywheel/schemas/context-map.md` holds.
