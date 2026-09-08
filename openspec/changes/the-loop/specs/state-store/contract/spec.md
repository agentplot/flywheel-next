## Purpose

The one door between the data plane and durable, shared storage: the operations
the engine may ask for, the guarantees each must give, and the rule that a
profile is admitted only by binding every one of them.

## ADDED Requirements

### Requirement: The state store offers exactly the operations of 125

The state store SHALL offer exactly these operations and the engine SHALL need
no others: read an object's evidence; write an effect; take, renew and release a
lease on an object; present the plan's decisions and receive the operator's
response; notify a host that state has changed; list the objects in a scope;
serve the status view (125). An engine that needs a further operation SHALL be a
change to this contract, stated as one.

#### Scenario: No caller reaches storage another way
- **WHEN** any part of the machinery reads or writes durable state
- **THEN** it does so through one of those operations and through nothing else
  (125)

#### Scenario: Read names its point and writes nothing
- **WHEN** an object's evidence is read
- **THEN** the evidence returned is as of a point the store names, and the read
  makes no write; reading twice with nothing changed returns the same evidence
  (126)

### Requirement: A write is durable, atomic and single-writer

What a write reports as written SHALL survive the loss of every host, all at
once, without warning (133). A write SHALL be wholly applied or not applied, and
no reader SHALL ever see half of one (135). Two writers of one object SHALL NOT
both succeed; the loser SHALL learn that it lost and SHALL read again before
deciding anything (134).

#### Scenario: The loser of a race reads again
- **WHEN** two hosts write the same object from the same base
- **THEN** exactly one write is applied, the other is refused, and the refused
  writer re-reads before it decides anything further (134)

#### Scenario: No reader sees a partial write
- **WHEN** a reader reads while a write is in progress
- **THEN** it sees the state before the write or the state after it, never a
  state between (135)

#### Scenario: A reported write survives the loss of every host
- **WHEN** every host is lost after a write was reported as written
- **THEN** the written state is readable again with no host of the operator's
  running (133, 132)

### Requirement: An effect is written with an identity and a repeat is not a second write

Every effect SHALL be written with an identity of its own. A repeat of an effect
already written SHALL change nothing, SHALL NOT be an error, and SHALL NOT be
reported as a second write (127).

#### Scenario: The same effect written twice
- **WHEN** an effect with an identity already present is written again
- **THEN** nothing changes, no error is raised, and the run record shows one
  write and not two (127)

### Requirement: A lease is taken, renewed and expired by a stated rule

A lease on an object SHALL be taken, renewed while its holder works, and
released by its holder or expired by a stated rule (128). Two would-be holders
of one object SHALL NOT both hold it (128).

#### Scenario: Two would-be holders, one lease
- **WHEN** two hosts attempt to take the lease on one object at the same moment
- **THEN** exactly one holds it and the other reads the holder and moves on (128)

### Requirement: A response is presented, received and applied exactly once

Decisions SHALL be presented to the operator and the response SHALL come back
attributed to the decision it answers (129). A response that arrives twice SHALL
be applied once, however many times it is delivered and whatever restarts happen
between its giving and its application (129, 137). A response that cannot be
applied SHALL be handed back to the engine and never dropped (129, 6).

#### Scenario: A response delivered twice
- **WHEN** the same response arrives a second time, before or after a restart
- **THEN** it takes effect once, and the second delivery changes nothing (137)

#### Scenario: A response whose decision is gone
- **WHEN** a response arrives naming a decision that has been retracted
- **THEN** it is handed back as unapplicable, shown once under attention, and
  never dropped (6, 129)

#### Scenario: A document under review returns its annotation as the response
- **WHEN** a decision carries a document and the operator annotates it on the
  review surface the profile names
- **THEN** the annotation comes back as the response on that decision (129, 17)

### Requirement: Notify only shortens the wait

The state store SHALL tell a host that state has changed within a bound the
profile states, and without the host re-reading everything to find out (130). A
host that is never notified SHALL still converge by reading (130).

#### Scenario: A never-notified host converges
- **WHEN** no notification reaches a host
- **THEN** the host still reaches the same state by reading on its own interval,
  within the bound the profile states (130)

#### Scenario: Notification names what moved
- **WHEN** a notification arrives
- **THEN** the host re-reads only the objects it names, not the whole store (130)

### Requirement: List and the status view make a forgetful engine complete

The state store SHALL enumerate the objects in a stated scope, so that an engine
which remembers nothing can find everything it must act on (131). It SHALL serve
the status view from the same state the engine reads, readable with no machinery
running anywhere (132). Every state the engine decides upon SHALL be derivable
from what read and list return; nothing the store holds privately SHALL decide
behaviour (136).

#### Scenario: An engine that remembers nothing finds its work
- **WHEN** a host starts with no memory of what existed
- **THEN** list returns every object in its scope and the host acts on all of
  them (131)

#### Scenario: The status view with nothing running
- **WHEN** no host of the operator's is running
- **THEN** the status view is still readable, and it says as of when (132, 145)

### Requirement: A profile is admitted only by a complete binding

A machine definition SHALL name the evidence it reads and the effects it writes
by abstract name only (138). A profile SHALL supply a binding from every such
name to the operations of its own storage, reviewable as data, and the
definitions SHALL NOT change when the profile changes (139). An engine SHALL run
unchanged against any profile whose binding is complete; a binding that leaves a
name unsatisfied SHALL NOT be a profile (140).

#### Scenario: An incomplete binding is refused
- **WHEN** a profile leaves one evidence or effect name unbound
- **THEN** it is refused as a profile and the unbound name is reported (140)

#### Scenario: The same machines under a second profile
- **WHEN** a second profile's binding is complete
- **THEN** the engine runs the byte-identical machine files against it with no
  change to any definition (139, 140)
