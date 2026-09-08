## Purpose

Testing as data: scenarios that live beside the definitions they check, run
against a stand-in and against the real profile, with only the session binding
and the recorded line-and-place effects standing in for the world.

## ADDED Requirements

### Requirement: Every machine is testable on its own with no live service

Every machine SHALL be testable on its own against a stand-in state store, with
no live service of any kind (92). Given a described state of the stores, the
model's decisions SHALL be assertable without starting anything (84).

#### Scenario: A machine asserted from a described state
- **WHEN** a described state of the stores is given and a tick is run against
  the stand-in
- **THEN** the transitions, the effects and the decisions can be asserted, with
  no network, no repository and no agent (84, 92)

### Requirement: A scenario is data, and it lives beside the definitions

A scenario SHALL be data: given this evidence, when this tick or event, then
these transitions, these effects and these decisions (94). Scenarios SHALL live
beside the definitions they check (94). A scenario SHALL be renderable
afterwards as a trace a person reads (95).

#### Scenario: A scenario runs and renders — mirrors S16
- **WHEN** a scenario file is run
- **THEN** it validates against the scenario schema, it runs with no live
  service, its transitions, effects and decisions are asserted in order, and a
  trace file named for it is written listing the ticks, the guards, the
  transitions, the effects, the decisions and their numbers (94, 95)

#### Scenario: A scenario states what it exercises
- **WHEN** a scenario is added
- **THEN** it names the requirements it exercises, and a number that names no
  requirement fails the check (94)

### Requirement: Only the session binding and the recorded effects stand in

The whole machinery SHALL run with sessions replaced by a stand-in that plays a
scenario's scripted exits, so that seeding a scenario exercises the stores, the
engine, the git effects, the plan and the page with no agent running (93). The
stand-in SHALL play every exit, offer and refusal by running the same command a
session reports through, so the state store sees the same records (67, 93).
While the phase performs no construction, the line-and-place effects MAY also be
bound to the recorded stand-in (93a), and a scenario whose assertion is about a
real take, merge, rebase, conflict or landing SHALL NOT be run against it (93a).

#### Scenario: A scripted exit goes through the real command
- **WHEN** the stand-in plays a session's exit
- **THEN** it runs the same reporting command a real session runs, and the
  assertion is about what that command wrote and never about the script (67, 93)

#### Scenario: A scenario about a real merge is not run against a recorded workspace
- **WHEN** the suite is run on a host whose line-and-place effects are recorded
- **THEN** the scenarios asserting a real take, merge, rebase, conflict or
  landing are excluded, and the run record names the subset that ran (93a)

### Requirement: Instructions are data, and a prompt can be rendered without starting a session

The schemas an artifact must satisfy, the instructions for writing each
artifact, and the skill for each session type SHALL be data, versioned like
anything else, and a session SHALL be given the versions in force when it starts
(88). A session's inputs SHALL be enumerable and closed — the schema
instruction, the type skill, its work order, and the artifacts of the change it
works — and nothing else SHALL reach it (89). No instruction text SHALL exist in
the engine and no engine behaviour SHALL depend on an instruction's wording
(119). Each instruction SHALL be versioned so a session started before a change
and one started after can be told apart (123), and the instructions SHALL live
in the shipped set and reach every host with it (91). A test SHALL be able to
render the exact prompt a given scenario would produce, without starting a
session (90), and to show, for a given instruction version and a scenario, what
a session would be asked to write (124).

#### Scenario: The prompt for a scenario and an instruction version
- **WHEN** a session type, an instruction version and a scenario are named
- **THEN** the work order that would be handed in is rendered whole, with no
  session started, and it holds the schema instruction, the type skill, the work
  order and the change's artifacts and nothing else (88, 89, 90, 124)

#### Scenario: Two instruction versions are told apart
- **WHEN** an instruction is changed and the prompt is rendered before and after
- **THEN** the two renderings name different instruction versions, so a session
  started before the change and one started after can be told apart (123, 91)

### Requirement: The suite runs against the stand-in and against the real profile

The suite SHALL run against the stand-in state store and then against the real
profile in a sandbox, with the machine files byte-identical between the two and
their hash written into the run record (168). A profile SHALL be admitted when
every scenario that applies to it passes and its binding is found complete (140,
168).

#### Scenario: The same files against both paths
- **WHEN** the suite is run against the stand-in and then against a local
  sandbox of the real profile
- **THEN** the same scenario files and the same machine files are used, their
  hash is recorded and equals the hash of the definitions directory the binary
  was built from, and both runs pass (168)

#### Scenario: The contract set runs before the domain loads
- **WHEN** the operations and guarantees of the state store contract are
  exercised
- **THEN** they are exercised over a toy machine sharing no atom with the
  flywheel, so the profile is tested before any domain definition is loaded
  (92, 140)

### Requirement: A second host in a scenario is a real process

A scenario that names more than one host SHALL be able to start, lose,
disconnect and return a second host as a process of its own, with its own root,
so that the multi-host rules are proved on one computer (147, 151, 232).

#### Scenario: Two hosts on one laptop — mirrors S13, S17 and S18
- **WHEN** a scenario starts a second host, loses it, or cuts its route
- **THEN** the stale-and-takeover path, the lease race and the disconnected
  reconciliation are each exercised against the real store, on one computer
  (147, 150, 151, 162, I15)

### Requirement: Every scenario carrying a response also runs at a phone viewport

Every scenario that carries an operator's response SHALL run at a 390px viewport
as well as at the desktop's (314). The pass SHALL assert that the decision's
number and its answers are reachable by tap with nothing behind a hover or a
keyboard, that the answer is given through the same tool the reply grammar
calls, and that a reload shows the answer recorded with who gave it and when
(311, 193, 310, 153, 154).

#### Scenario: A response given at 390px — mirrors S01
- **WHEN** a scenario carrying a response is run at a 390px viewport
- **THEN** the decision is answerable by tap, the response is recorded through
  the tool catalogue, and a reload shows it recorded with who gave it and when
  (314, 311, 310, 153)

#### Scenario: The same scenario at the desktop viewport
- **WHEN** the same scenario is run at the desktop's viewport
- **THEN** it passes the same assertions, so nothing the phone answers is
  missing on the desktop (306, 314)
