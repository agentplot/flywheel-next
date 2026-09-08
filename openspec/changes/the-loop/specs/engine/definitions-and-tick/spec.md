## Purpose

The generic part of the machinery: it loads the machine definitions, evaluates
their guards over evidence, chooses transitions, performs effects idempotently,
and does so on a tick that is the only clock the flywheel keeps.

## ADDED Requirements

### Requirement: The shipped definitions live in the binary and are named by the set version

The machines, profiles and schemas the flywheel ships SHALL be core definitions
carried inside the binary, never edited by an instance, each carrying its
own version, and all of them named by the release's set version (223, 224, 83).
Initialization and repository creation SHALL record the set version they used
(208).

#### Scenario: A host proves which definitions it ran
- **WHEN** a host performs any effect
- **THEN** the run record names the set version the binary carries, and that
  version names the version of every core machine it evaluated (224)

#### Scenario: An instance cannot edit a core machine
- **WHEN** a file under the instance's blueprints prefix would override a
  core machine
- **THEN** the machinery refuses it and reports the refusal, because a core
  machine ships with the release (223)

### Requirement: An instance's own types are read from the blueprints

Unit types and elaboration types an instance adds or overrides SHALL be read
from the blueprints repository at the shared line, and a type composed only of
existing predicate and effect atoms SHALL require no code change and no rebuilt
host (57, 85). An object SHALL record the version of the extensible machine it
runs under, and a type change SHALL NOT move an object already in flight (57,
224).

#### Scenario: A type added with no code change
- **WHEN** the operator commits a new unit type file composed only of existing
  atoms to the blueprints' shared line
- **THEN** every host loads it at its next fetch and can run a unit under it,
  with no binary changed and no host restarted (57)

#### Scenario: A type version does not move work in flight
- **WHEN** a type file's version moves while a unit of that type is in flight
- **THEN** that unit continues under the version its record holds, and the
  record still names that version after the change (57, 224)

### Requirement: The engine names nothing of the domain

The engine SHALL hold no name of an intent, elaboration, bolt, unit, work item,
claim or verdict, and no string of the atoms file (86, 87, I13). Every evidence
and effect a definition names SHALL be an abstract name a profile binds; no
definition SHALL name a store, a service, a path or a field (138, I13).

#### Scenario: The domain boundary is checkable
- **WHEN** the engine's sources and tests are searched for those seven names as
  whole words, for any atom name, and for any instruction text
- **THEN** no match is found, so no instruction text exists in the engine and no
  engine behaviour depends on an instruction's wording (86, 119, I13)

#### Scenario: The engine runs a machine that shares no atom with the flywheel
- **WHEN** the engine is given a toy machine naming none of the flywheel's atoms
- **THEN** it loads it, evaluates its guards and fires its transitions, proving
  it knows nothing of what the flywheel is about (86)

### Requirement: An effect is performed only when its proof is absent, and a repeat changes nothing

Every effect SHALL name the evidence that proves it was done, and the machinery
SHALL perform an effect only when that proof is absent (73). Every effect write
SHALL carry an identity of its own; a repeat of an effect already written SHALL
change nothing, SHALL NOT be an error, and SHALL NOT be reported as a second
write (127).

#### Scenario: A slow start is slow, not failed — mirrors S06
- **WHEN** starting a session takes two minutes and the command that started it
  has already returned
- **THEN** the machinery judges the start by evidence that the session exists,
  retries under the same deterministic name, has the second and third starts
  refused as duplicates of that name, ends with one session, and reports nothing
  failed (72, 73)

#### Scenario: A repeat is a no-op write, not a skipped act — mirrors contract/write-effect
- **WHEN** the proof of a performed effect is lost from the world and the next
  tick performs the act again
- **THEN** the act is performed a second time because its proof is absent, the
  write it makes carries the same identity, the state store changes nothing for
  it, and the run record reports one write and not two (73, 127)

### Requirement: Reading twice with nothing changed writes nothing

Every object's state SHALL be derivable from durable stores at any moment, and
nothing held only in a process's memory SHALL decide behaviour after that
process restarts (75, I14). Reading the same stores twice with nothing changed
SHALL produce the same conclusion and no writes (78).

#### Scenario: A quiet tick is silent
- **WHEN** a tick runs and no evidence has moved since the last
- **THEN** no transition fires, no effect is performed and no write is made (78)

#### Scenario: A restart changes no state and no rail — mirrors S05
- **WHEN** the machinery is stopped and started again mid-day
- **THEN** every object is in the state it was, every running session is still
  read as running from evidence rather than memory, and the rail is identical
  (7, 75, I7, I14)

### Requirement: The tick is the only clock

A host SHALL be one long-lived process its platform's launcher starts, and every
timed behaviour of that host, adapters included, SHALL be a guard on its tick;
nothing else SHALL keep time (231). A run missed while the host was down SHALL
be caught up on the next tick, and the idempotent key SHALL make the catch-up
write nothing twice (231, 111).

#### Scenario: A missed cadence is caught up once
- **WHEN** a host is shut for a day and started again, and curation's cadence
  should have fired three times while it was down (110)
- **THEN** the next tick charges curation once, under the idempotent key, and
  nothing is written twice (231, 111)

#### Scenario: Nothing happens between ticks
- **WHEN** the clock is advanced past the interval of a timed behaviour and no
  tick is run
- **THEN** nothing happens: no transition fires and no effect is performed
- **WHEN** the next tick runs
- **THEN** the behaviour fires once (231)

### Requirement: Line and place effects may be recorded while no construction runs

While the phase performs no construction, the effects that change a line of work
or a place to work in (42) SHALL be bound to a recorded stand-in that writes the
evidence each effect's proof reads and touches no repository (93a). The machines
and their proofs SHALL NOT change under that binding, and it SHALL be stated in
the manifest like any other binding (93a, 139).

#### Scenario: A machine ticks over recorded evidence
- **WHEN** an approved unit's work item reaches the state that prepares a place
- **THEN** the recorded binding writes the evidence the place's proof reads, the
  machine advances, and no repository is touched (93a)

#### Scenario: The binding is named, not inferred
- **WHEN** a host starts
- **THEN** it reads which binding it runs from the manifest and records it in
  the run record, so a reader can tell a recorded effect from a performed one
  (93a, 139)
