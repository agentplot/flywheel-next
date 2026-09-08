## Purpose

The complete binding of the state store contract to git alone: every durable
fact a commit on one state repository, the git host the only central service,
and no tracker anywhere in the path.

## ADDED Requirements

### Requirement: All state is files in git and the git host is the only central service

Every piece of durable state SHALL be a file in a git repository, and the git
host SHALL be the only central service (160). Issues, milestones and boards MAY
exist for people, and the machinery SHALL neither read nor write them (C.2).

#### Scenario: The machinery never reads the git host's issues
- **WHEN** the machinery needs any evidence
- **THEN** it reads files of the state repository or the blueprints repository,
  and no issue, milestone, board or review of the git host (C.2, 160)

#### Scenario: The git host is depended on for three things only
- **WHEN** the profile runs
- **THEN** it uses the git host to accept one update to a branch at a time, to
  reject an update whose base is stale, and, when the operator has configured
  one, to call a URL when a branch moves; it depends on no other service of the
  host (160, 162, 166)

### Requirement: A change of state is a commit that carries its reason and evidence

A change of state SHALL be a commit (161). A commit that reaches the shared line
SHALL be the fact; a commit that has not SHALL be a local intention (161). Every
commit the machinery makes SHALL carry the effect's identity, its reason and the
evidence it was based on, and history SHALL be the audit record with nothing
else kept for that purpose (167, 79, 127).

#### Scenario: A repeat is recognised before it is written
- **WHEN** the machinery would perform an effect whose identity already appears
  in the shared line's history
- **THEN** no commit is made and no second write is reported (127, 167)

#### Scenario: The state write carries the response that caused it
- **WHEN** a transition fires on the operator's response
- **THEN** the new state and the response's identity are one commit, so the
  response takes effect exactly once whatever is delivered twice or restarts in
  between (137, I2)

### Requirement: The push is the compare-and-swap

Two hosts that try to change the same object at the same time SHALL NOT both
succeed, because the git host rejects an update whose base is stale (162, I15).
The loser SHALL fetch, rebase its one-file commit and push again; after three
rejections it SHALL report and re-read (162, 134).

#### Scenario: Two hosts race for one object — mirrors S17
- **WHEN** two hosts see the same approved unit and both try to take it
- **THEN** exactly one succeeds, the other learns it lost and moves on, and the
  object is worked once (134, 162, I15)

#### Scenario: The operator's own commit wins a content conflict
- **WHEN** the operator commits a change to an object's file by hand while a
  host is about to write the same file, and the host's rebase conflicts on
  content
- **THEN** the host discards its local commit, re-reads the object, and treats
  the operator's commit as the response (3, 164, I15)

### Requirement: A lease is a branch of its own, taken by a landing commit

A lease SHALL be taken by a commit that lands, renewed while the host works, and
expired by a stated rule (163). Leases and heartbeats SHALL be kept off the
shared line, so that renewals add nothing to its history and history stays the
audit record (163, 167). The expiry rule SHALL be that the lease's renewal is
older than 24 hours, or that the holder's host decision was answered takeover
(128, 163, 150).

#### Scenario: Renewals do not grow the audit history
- **WHEN** a host renews its leases every minute for a month
- **THEN** the shared line's history holds no commit for any of those renewals
  (163, 167)

#### Scenario: A lease expires by the stated rule
- **WHEN** a lease's renewal is older than 24 hours, or the operator answers
  takeover on the holder's host decision
- **THEN** the lease is expired and another host may take it; without one of
  those, no host takes it by racing (150, 163)

### Requirement: A disconnected host keeps what it owns and reconciles on return

A host that cannot reach the git host SHALL keep ticking the objects whose
leases it holds, up to the expiry, and SHALL commit locally (151, 165). It SHALL
NOT take a new lease, start a session for an object it does not hold, deliver to
any sink, or expire another host's leases (151). On reconnect it SHALL push its
lease renewals first — a rejection meaning the object was taken over, so it ends
its own work on that object, reports, and discards its local commits on it — and
SHALL then rebase and push its remaining commits (165).

#### Scenario: A host's network drops for an hour — mirrors S18
- **WHEN** a host loses its route, finishes the work it owned and commits
  locally, then reconnects
- **THEN** its renewals land, its commits land after them, and nothing of
  another host's is lost (151, 165)

#### Scenario: A write made while disconnected is an intention
- **WHEN** an effect is written while the host has no route
- **THEN** the effect is reported as pending and not as written, the run record
  says it has not landed, and it is reported as written only when its push lands
  (133, 161)

#### Scenario: Unpushed commits found at start
- **WHEN** a host starts and finds commits on the shared line's local branch
  that never landed
- **THEN** it pushes them by the same rebase-retry before its first tick,
  discarding those on an object whose lease it no longer holds, and infers
  nothing from the working tree (I14, 165)

### Requirement: Every host fetches before it decides, and notify only shortens the wait

Every host SHALL fetch and integrate the shared line on every notification, on a
bounded interval, and before every tick; no host SHALL decide on a read older
than that bound, and no person SHALL run the sync by hand (165). Hosts SHALL
learn of new state without reading the whole history each time, within a stated
latency bound (166).

#### Scenario: The bounded poll is the default on a laptop
- **WHEN** a host runs on the operator's own computer with no address reachable
  from outside
- **THEN** it learns of new state by a bounded poll of the shared line's head
  within the stated bound, and the git host's call is used only when the
  operator has chosen to publish an address for it (46, 130, 166)

#### Scenario: A fetch names what moved
- **WHEN** a fetch brings new commits
- **THEN** the host reads only the object files those commits changed (166)

### Requirement: The operator's own act on the state is the response

The operator acting directly on the state where it is kept SHALL be the
response, and the machinery SHALL treat it as the response at its next read
(3, 164). The response SHALL be recorded with the object it concerns, the
decision it answers, who gave it and when, before any work follows from it
(153), and the operator SHALL be able to tell it was recorded without asking
anyone (154).

#### Scenario: The operator edits a state file by hand — mirrors S19
- **WHEN** the operator commits an edit to an object's state file
- **THEN** the next pass on every host treats it as the response, applies it
  once, and records who gave it and when (3, 153, 164)

#### Scenario: The operator can tell the response landed
- **WHEN** a response is given from a phone or the page
- **THEN** the operator is shown that it was recorded, without asking anyone,
  once the write has landed (154)

### Requirement: The status view is readable with no host running

The status view SHALL be served from the same state the engine reads and SHALL
be readable with no machinery running anywhere, stating as of when (132, 145).
No rendering of the plan SHALL be committed (15).

#### Scenario: The status page six hours after the last host stopped — mirrors S20
- **WHEN** no host has run for six hours and the operator opens the status view
  from a phone
- **THEN** it shows the state as of the last landed commit and says so (132,
  145)

#### Scenario: The plan is served and never stored
- **WHEN** the plan is shown on any surface
- **THEN** it is derived from the state at that moment and no rendering of it
  exists in the repository (15)
