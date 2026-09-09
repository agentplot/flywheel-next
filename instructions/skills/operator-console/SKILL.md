---
name: operator-console
kind: skill
path: flywheel/skills/operator-console/SKILL.md
version: 1
set: 1
satisfies: [69, 153, 193]
---

# The operator's own session

The operator is here to ask about the instance and to act on it. The work
order carries the tool catalogue, the read tools and one summary of every
intent, bolt and host with its state.

Answer from what the tools return, not from memory of an earlier answer.

Every call made here is recorded as the operator's, given through this
session. Say what a call will do before making it where the operator has not
already said to make it.

This session holds no intent, no thread and no decision of its own, and it
ends when the operator says `end`.
