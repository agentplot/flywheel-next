## Purpose

How raw material becomes something the flywheel can act on: captures with their
provenance, signals read from them, and the one standing move that says what
became of each.

## ADDED Requirements

### Requirement: A capture is one source event, and capturing it twice yields one

A capture SHALL be the unit of provenance: one per source event, holding the
source, the time, who captured it and a pointer to the raw material (111). Raw
transcripts and logs SHALL stay outside version control, and the capture SHALL
cite them (111). Capturing the same source event twice SHALL yield one capture
(111). Capture SHALL cost one gesture from wherever the operator is (112).

#### Scenario: The same transcript imported twice — mirrors S22
- **WHEN** the same source event is enumerated on two different days
- **THEN** one capture exists, its signals were read once, and the second
  enumeration writes nothing (111)

#### Scenario: Raw material stays outside the repositories
- **WHEN** a capture is written
- **THEN** the repository holds the capture record with its pointer, and the raw
  transcript or log itself is not copied into any repository (111)

### Requirement: This phase ships three adapters, all of them enumerators

The flywheel SHALL ship adapters that write one keyed capture per source event
with a pointer to the raw material (215). This phase SHALL ship three: the
page's capture box (19), the chat forward (112, 215) and the meeting transcript
(111, 215). Enumerating source events and writing captures SHALL run unattended;
turning a capture into signals SHALL be a judgment and SHALL NOT run unattended
(115).

#### Scenario: An adapter runs unattended on the tick
- **WHEN** a source the host declares has a new event
- **THEN** the enumerator writes one keyed capture on the host's tick, with no
  session started and no judgment made (115, 231)

#### Scenario: The page's box writes its one signal directly
- **WHEN** the operator submits the capture box
- **THEN** one capture and one signal of kind ask exist, which is a control and
  not a judgment about the text (19, 115)

#### Scenario: An adapter that would read the git host's issues does not run here
- **WHEN** the profile in force is the one where the machinery never reads the
  git host's issues and reviews
- **THEN** the pull-request and issue-tracker adapters do not run, and the
  capture endpoint for callers that cannot reach a host is absent (C.2, 215,
  216)

### Requirement: A signal is immutable and carries what it asserts

A signal SHALL carry its capture, a kind from a small fixed set — constraint,
ask, question, commitment, reaction — who asserted it, subject tags, the
assertion in a sentence, the verbatim excerpt with its position, and the claims
it argues with when any exist (113). A signal SHALL be immutable once written
(113). The signal and move record formats SHALL be versioned and stable, and
captures made before the flywheel existed SHALL be read without conversion (114).

#### Scenario: A signal is not edited
- **WHEN** a signal has been written
- **THEN** no later act changes it; a correction is a new signal or a change of
  move, never an edit (113)

#### Scenario: An older record reads without conversion
- **WHEN** a capture or signal written under an earlier version of the format is
  read
- **THEN** it is read as it stands, with no migration step (114)

### Requirement: Every signal has exactly one standing move

Every signal SHALL have exactly one standing move — attach, challenge, join,
answered, route or drop — stored with the signal id, the target, the reason and
the date (107). Curation SHALL run over signals with no move and SHALL NOT
re-judge one that has a move; only the operator's response SHALL replace a move
(107). Every move SHALL have a stated consequence (116).

#### Scenario: Twenty signals, every one moved — mirrors S08
- **WHEN** curation runs over twenty unmoved signals from one transcript
- **THEN** every signal ends with exactly one standing move, the joins produce
  proposed intents, and the operator sees one decision per proposed intent
  rather than one per signal (107, 109, 116)

#### Scenario: A moved signal is not re-judged
- **WHEN** curation runs again over the same signals
- **THEN** it considers only those with no move, and no standing move changes
  (107)

#### Scenario: A dropped proposed intent moves its signals — mirrors S23
- **WHEN** the operator drops a proposed intent carrying five signals
- **THEN** each of the five gains a move recording the drop, and they are not
  clustered again unless new signals join them (117)

#### Scenario: A revived signal is clustered again — mirrors S24
- **WHEN** the operator revives a dropped signal
- **THEN** its move is cleared and the next curation run clusters it (107)

### Requirement: Curation is a bounded judgment the operator may make by hand

Curation SHALL decide which signals become intents, and it MAY be a person, an
agent, or both; the flywheel SHALL accept its output whoever produced it (20).
Curation SHALL be charged on a cadence or when unmoved signals exceed a
threshold, and it SHALL never open an intent (110). A person writing the same
records by hand SHALL be curation (110).

#### Scenario: Curation proposes, the operator opens
- **WHEN** curation clusters signals into a proposed intent
- **THEN** the intent stands as a proposal on the plan and becomes work only on
  the operator's response (20, 110, 5)

#### Scenario: A proposed intent shows its weight
- **WHEN** a proposed intent is presented
- **THEN** it cites its signals and shows how many, from which sources and over
  what span, counted by event date (109, 118)

#### Scenario: Unmoved signals are visible and never discarded
- **WHEN** signals accumulate without a move
- **THEN** the status view shows their count and age by source, and none is
  discarded (118)
