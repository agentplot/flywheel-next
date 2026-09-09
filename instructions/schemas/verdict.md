---
name: verdict
kind: schema
path: flywheel/schemas/verdict.md
version: 1
set: 1
satisfies: [100, 101]
---

# A verdict

One judgment that a repository satisfies a claim, at a version of the claim,
made by an agent and not by a computation.

It carries the claim and its version, the repository, the judgment —
satisfied, not satisfied, or not applicable — the inputs it was made from, and
the reasoning in enough detail that a reader can disagree with it
specifically.

Not applicable is a judgment like any other, made once and reused; it is not a
way of declining to judge.

Invalid: a verdict with no inputs, a verdict that judges a claim version other
than the one it names, a verdict whose reasoning restates the claim instead of
weighing it, and a verdict written by the session that did the work the
judgment is about.
