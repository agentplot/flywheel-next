---
name: design-conclusion
kind: instruction
path: flywheel/instructions/design-conclusion.md
version: 1
set: 1
satisfies: [97, 120, 121]
---

# Settling a design conclusion

A conclusion is settled when it is written down where the next reader will
find it, and it is not settled until then.

Write in one commit:

- the chapter that explains the conclusion, in the voice of the destination
  — the system as it will be, not the change that gets there;
- the claim it adds or amends, in the intent's specification delta, included
  in that chapter by anchor so the prose and the statement cannot drift (97);
- the context map, moved to match what the chapter now says.

One commit, not three. A chapter whose claim lands later has explained
something the flywheel cannot check; a claim whose chapter lands later is a
statement no one can read.

Amend rather than add when the destination already has a claim about this.
An amended claim's version moves because its text moved, and everything that
cited it is asked again; that is the point.

If the conclusion settles nothing a repository could ever be judged against,
write the chapter and say in the exit that it yields no claim. An invented
claim is worse than none: it costs a verdict per repository in scope, for as
long as it stands.
