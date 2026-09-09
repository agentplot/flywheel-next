---
name: claim-granularity
kind: instruction
path: flywheel/instructions/claim-granularity.md
version: 1
set: 1
satisfies: [100, 120]
---

# The granularity of a claim

Write a claim at the granularity of the destination, not of the work. Three
tests, all of which a claim passes:

1. **It survives a rewrite of the code.** Throw the implementation away and
   build it again some other way; if the claim would have to be rewritten
   too, it was about the code and not about the system.
2. **It has an observer.** Someone — a person, a caller, an operator — would
   want to know if it stopped holding. If no one would notice or care, it is
   not a claim.
3. **It is judgeable from the repository alone.** An agent reading the
   repository can say satisfied, not satisfied, or not applicable, and say
   why. A statement no one could judge without asking a person is not a
   claim.

None of these is a claim: a file, a colour, a setting, a plugin, a step of a
pipeline, a library version, a function name. They are true today and mean
nothing tomorrow.

Every claim costs a verdict in every repository in scope, forever. Write the
coarse statement the fine work serves, and let the fine work cite it.

Each claim carries at least one scenario saying how one would know it holds.
A claim without one is a wish.
