## Purpose

Which host acts on which object, how that ownership is visible, and what happens
when a laptop closes: declarations, leases, heartbeats and the away window.

## ADDED Requirements

### Requirement: A host declares what it takes and takes nothing else

A host SHALL declare in the manifest what it takes: kinds of object,
repositories, unit types, and whether it presents (149). It SHALL take leases
only within its declaration (149). An object that no host's declaration covers
SHALL be a decision under attention and never a silent wait (149).

#### Scenario: An object no declaration covers — mirrors X05
- **WHEN** work exists of a type no host declares
- **THEN** a decision under attention says so, nothing waits silently, and when
  the manifest is changed to cover it a host takes it on the next tick (149,
  150)

#### Scenario: A host does not reach outside its declaration
- **WHEN** an object outside a host's declaration is ready to move
- **THEN** that host takes no lease on it and performs no effect for it (149)

### Requirement: Every object is owned by at most one host, and the owner is visible

Every object SHALL be owned by at most one host at a time, through a lease, and
the owner SHALL be visible (150, I11). Two hosts SHALL never work the same
object (150). A host that goes away SHALL leave its objects visibly stale, and
another host SHALL take them over only when the lease has expired by the stated
rule, never by racing (150). The rule MAY name the operator's response for work
that has a session behind it (150).

#### Scenario: A host loses power mid-work — mirrors S13
- **WHEN** one of two hosts, not declared intermittent, loses power while
  holding an object with a session behind it, and work waits on that object
- **THEN** the status view shows the object and its host stale; past the long
  bound the host-gone decision is raised; on the operator's takeover the other
  host starts a fresh attempt; the returning host reads that its lease was
  replaced, ends its own session for that object and reports; and one session is
  running at the end — never two on one object and never taken by racing (150,
  150a, I11)

#### Scenario: The owner is on the status view
- **WHEN** any object is read on the status view
- **THEN** it shows which host holds it, which host runs its session, and
  whether that host is alive (141, 150, I11)

### Requirement: An intermittent host is away, not lost

A host MAY be declared intermittent, and a laptop SHALL be intermittent by
default (150a). An intermittent host past its stale window SHALL be shown as
away, with since when, and SHALL raise no attention line (150a). Its leases
SHALL stand, its sessions SHALL be neither stalled nor lost while it is away,
and their stall clocks SHALL pause (150a). The takeover decision SHALL be raised
for an away host only when a numbered decision or approved work is waiting on it,
or when the long bound passes (150a). On its next heartbeat it SHALL be alive
again with nothing to answer (150a).

#### Scenario: A laptop closed overnight
- **WHEN** the operator closes the laptop past the stale window and nothing is
  waiting on it
- **THEN** the host shows as away with since when, no attention line is raised,
  its leases stand and its sessions' stall clocks are paused (150a)

#### Scenario: Work waiting on an away host
- **WHEN** approved work or a numbered decision is waiting on an away host
- **THEN** the takeover decision is raised (150a, 150)

#### Scenario: The laptop returns
- **WHEN** the away host heartbeats again
- **THEN** it is alive, its leases and sessions continue, and nothing is asked
  of the operator (150a)

### Requirement: A host may declare the operator as its session binding

A host MAY declare the operator as its session binding in the manifest, and a
host so bound SHALL start no agent (93b, 69). Under it the machinery SHALL
charge a session as it always does — a place prepared, a work order rendered,
the session recorded — and the plan and the status view SHALL show the session
as the operator's to run (93b, 89). The operator SHALL report through the same
command a session reports through, and the exits, offers and refusals SHALL be
the same records, so nothing downstream tells the two apart (93b, 67). A session
charged this way SHALL be a with-operator session for every rule that turns on
the type, and the machinery's own sessions, curation among them, SHALL be the
operator's under the same rule (93b, 25, 110). A session SHALL be given one job,
one place and a bounded goal, and its only outputs to the machinery SHALL be the
fixed exits — done with deliverables, blocked on a question, offering a finding,
offering a chore, stalled — with anything else refused (65, 66).

#### Scenario: An approved elaboration is the operator's to run
- **WHEN** an elaboration is approved on a host whose manifest names the
  operator as its session binding
- **THEN** the place is prepared and the work order rendered, the session record
  names its place and work order, the plan shows it as the operator's to run, no
  agent process is started, and the session reads present and working until the
  operator reports (93b, 89)

#### Scenario: The operator's exit is a session's exit
- **WHEN** the operator runs the exit command with deliverables for that session
- **THEN** the object's thread carries the same exit record a scripted exit
  would write, and the elaboration advances as after any session's exit (93b,
  67)

#### Scenario: A report outside the fixed exits is refused
- **WHEN** a report arrives that is none of the five exits
- **THEN** it is refused and the refusal is recorded, because the exits are a
  closed set (65, 66)

#### Scenario: The with-operator rules apply and the binding is recorded
- **WHEN** a host so bound starts and a session it charged goes idle for a day
- **THEN** the run record names the operator binding, no finish-or-keep is
  offered, and the session ends only by the operator's dictation (93b, 25)

### Requirement: More than one host may run for one organization

More than one host MAY run the machinery for one organization at once, every one
working from the same shared line of every repository and the same central state
(147). There SHALL be one plan per organization, derived from the shared state,
and any host SHALL be able to serve it; exactly one presenter SHALL deliver it to
each sink at a time (148).

#### Scenario: Two hosts, one plan
- **WHEN** two hosts run for one organization
- **THEN** both derive the same plan with the same numbers, and each sink is
  delivered to by exactly one of them (147, 148, 15)

### Requirement: A disconnected host keeps what it owns and reconciles

A host that cannot reach the central service SHALL keep working what it already
owns, SHALL record what it does locally, and SHALL reconcile when it reconnects
(151). Every host SHALL fetch and integrate the shared line on every
notification, on a bounded interval, and before every tick, and no person SHALL
run the sync by hand (165).

#### Scenario: An hour without a route — mirrors S18
- **WHEN** a host's route is lost for an hour
- **THEN** it finishes what it owned and records it locally, takes no new lease,
  starts nothing new, delivers to no sink, and reconciles on reconnect (151,
  165)

### Requirement: The number of sessions running at once is bounded per host

The number of sessions running at once on a host SHALL be bounded by a setting
of that host (32). At the bound, ready work SHALL wait in a stated order, and
nothing SHALL be lost or started twice (32).

#### Scenario: Three items, a bound of two — mirrors S29
- **WHEN** two independent items and one dependent item are ready on a host
  whose bound is two
- **THEN** two run, the third waits in the stated order and starts only when its
  dependencies are merged and a slot frees, merges are recorded one at a time in
  ordinal order, and nothing is started twice across a restart (31, 32, 38, 73,
  93a)

#### Scenario: The bound is the host's own setting
- **WHEN** the manifest sets a different bound for a host
- **THEN** that host runs at most that many sessions at once, and other hosts
  are unaffected (32)
