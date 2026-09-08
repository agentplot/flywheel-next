## Purpose

What the machinery leaves behind so a person can see what it did and why: every
write with its reason and evidence, expected against delivered, and a status
view of the whole that needs nothing running to be read.

## ADDED Requirements

### Requirement: Every write is recorded with its reason and its evidence

Every write the machinery makes SHALL be recorded with its reason and the
evidence it was based on (79). History SHALL be the audit record and nothing
else SHALL be kept for that purpose (167).

#### Scenario: A transition is explicable after the fact
- **WHEN** an object has moved from one state to another
- **THEN** the record of that write names the transition, the reason, and the
  evidence values the guard read (79, 167)

#### Scenario: An effect names its identity
- **WHEN** an effect has been performed
- **THEN** the record carries the effect's identity, so a repeat is recognised
  and not written twice (127, 167)

### Requirement: Expected against delivered is the first thing a report shows

For every session, what was expected and what was delivered SHALL both be
recorded, and the difference SHALL be the first thing a report shows (80).

#### Scenario: A session delivers less than was expected
- **WHEN** a session exits done having produced two of the three deliverables
  its type expected
- **THEN** the record holds both lists and the report leads with the difference
  (80)

### Requirement: Problems with the machinery are reported, never filed as work

Problems with the machinery itself SHALL be reported to the operator through the
run record and SHALL never be filed as work (81). A refusal — an act the
machinery declines, a response it cannot apply, a call it will not make — SHALL
be recorded with what was refused and why (79, 81, 6).

#### Scenario: A refusal is recorded and surfaced
- **WHEN** the machinery refuses an act, such as a dictation asserting work was
  done
- **THEN** the refusal is written to the run record with the identity, the
  operation and the object, and it reaches the operator under attention rather
  than becoming a unit (4, 79, 81)

#### Scenario: A machinery failure is not turned into work
- **WHEN** the machinery cannot perform an effect for a reason of its own
- **THEN** it is reported through the run record, and no intent, unit or chore
  is created for it (81)

### Requirement: Notifications are routed by kind and nothing is visible only where it happened

Notifications SHALL be routed by kind to sinks the operator sets per kind (82).
A host running construction SHALL be silent by default, and nothing the
machinery notices SHALL be visible only on the host that noticed it (82).

#### Scenario: Something noticed on a silent host
- **WHEN** an event of a routed kind occurs on a host that presents no sink
- **THEN** it reaches the sinks the operator set for that kind, and the noticing
  host displays nothing of its own (82)

### Requirement: The status view is a projection of the same state, derived from list and read alone

The status view SHALL show every intent, elaboration, bolt, unit, work item and
session with its current state, grouped by state — queued, in progress, waiting
on the operator, done — and for each, which host holds it, which runs it and
whether that host is alive (141). For every state an object can be in, exactly
one source of truth SHALL prove it, and anything else showing that state SHALL
be a projection, written from the source and never read as truth (76, 142). The
status view SHALL be such a projection, never written by hand to look right
(142). It SHALL be central, one place for the whole instance, reachable
from a phone however many hosts run (143). It SHALL be derived from list and
read alone (146).

#### Scenario: The whole, grouped by state
- **WHEN** the operator opens the status view
- **THEN** every object appears once under queued, in progress, waiting on the
  operator or done, with its holder, its runner and that host's liveness (141)

#### Scenario: A projection that disagrees is rewritten from its source
- **WHEN** a projection disagrees with the state it projects
- **THEN** it is rewritten from the source on the next tick of that object, and
  the rewrite is reported with both values (77, 142)

### Requirement: Discussion about an object is part of its state

Discussion about an object — a question asked, an answer given, a note a session
left — SHALL be part of that object's state, and the status view SHALL show it
(144).

#### Scenario: A question and its answer stay with the object
- **WHEN** a session asks a question and it is answered
- **THEN** both are on the object, shown under it on the status view, and
  readable after the session is gone (144)

### Requirement: The status view is readable with no machinery running, and says as of when

The status view SHALL be readable with no machinery running anywhere (132, 141).
A status view read while nothing is running SHALL show the state as of the last
write that reached the central service, and SHALL say as of when (145).

#### Scenario: Read with nothing running
- **WHEN** the operator opens the status view while no host is running
- **THEN** it is readable, shows the state as of the last write that reached the
  central service, and says as of when; how a profile makes it readable is the
  profile's, and this phase's is mirrored by S20 in the git-only profile's spec
  (132, 145)

#### Scenario: Only one writer keeps the projection from racing
- **WHEN** more than one host could write the status projection
- **THEN** the host holding the rail's lease writes it and the others do not, so
  the projection never conflicts with itself (142, 148)
