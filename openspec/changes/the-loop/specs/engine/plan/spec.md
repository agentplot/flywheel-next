## Purpose

The single surface where everything awaiting the operator's response stands: how
a decision comes to exist, how it is numbered, how decisions group, and what the
tail shows since each sink last looked.

## ADDED Requirements

### Requirement: The plan is derived, never stored

The plan's content SHALL be a function of the current state of the system and
not of what any process remembers, so that the same plan results after a restart
(7, I7). The plan SHALL always be current: new material joins the standing plan
and nothing waits for a next one (8). No rendering of the plan SHALL be stored
(15).

#### Scenario: The plan survives a restart unchanged — mirrors S05
- **WHEN** the machinery restarts
- **THEN** the plan derived after the restart is identical to the one before it,
  with the same decisions and the same numbers (7, I7)

#### Scenario: New material joins the standing plan
- **WHEN** a decision becomes the operator's while the operator is not looking
- **THEN** it stands on the plan at once, and the operator who next opens the
  plan sees everything that stands (8)

### Requirement: A decision exists exactly while its state is active

A decision SHALL appear exactly when a choice becomes the operator's to make and
SHALL disappear when it is made or can no longer be made (9). Every decision
kind SHALL have exactly one creating condition and one retracting condition
(I3). Nothing the operator has not approved SHALL exist as work (5, I1).

#### Scenario: A decision is retracted when its choice is gone
- **WHEN** the condition that raised a decision no longer holds, without any
  response
- **THEN** the decision disappears from the plan and its register entry records
  that it was retracted by the machinery (9, I3)

#### Scenario: A proposal creates no work — mirrors S04
- **WHEN** a session offers a finding on its own intent, the plan shows it as a
  proposed elaboration, and the operator drops it
- **THEN** nothing was created before the response and nothing remains after it
  (5, 58, I1)

#### Scenario: A standing session is offered, never ended — mirrors S02
- **WHEN** a standing elaboration's session goes idle
- **THEN** the plan offers finish or keep, the session keeps running, and only
  the operator's response ends it (25, 26, I6)

#### Scenario: A thread whose work is done is offered for closing — mirrors S07
- **WHEN** every elaboration of an intent is done
- **THEN** the plan offers the intent's close, and only the operator's response
  closes it (22)

### Requirement: Every decision carries a number, given once and never reused

Every decision SHALL carry a short number unique in the organization, given once
and never reused (15). The page and the chat SHALL show the same number, and a
response SHALL name it (15, 18). A decision state left and re-entered SHALL be a
new decision with a new number, so an earlier reply cannot land on a question
that has changed (15).

#### Scenario: The same number on both surfaces
- **WHEN** a decision is delivered to the page and to the chat sink
- **THEN** both show the same number, and a response naming that number resolves
  to the same object and the same decision state (15, 18)

#### Scenario: A number is never reused
- **WHEN** a decision is retracted and a decision of the same kind on the same
  object is raised later
- **THEN** the later decision carries a new number, and the retracted number is
  never issued again (15)

### Requirement: Decisions group, and any one can be answered alone

Decisions SHALL be grouped so that "yes to all" is a meaningful answer for a
simple plan, and any single decision SHALL be answerable on its own (11).

#### Scenario: Yes to all expands into one response per decision
- **WHEN** the operator answers "yes all" against a delivery carrying three
  approve decisions
- **THEN** three responses are recorded, one per decision, each with its own
  delivery identity, and each applied exactly once (11, 137)

#### Scenario: One decision answered on its own
- **WHEN** the operator answers one number out of six standing
- **THEN** that decision is applied and the other five stand unchanged (11)

### Requirement: One response is enough

After a response is applied, everything that follows without a further decision
SHALL proceed by the machinery on its own; the operator SHALL never nudge (13).
A decision SHALL say what its yes starts, and after a yes the only things that
wait SHALL be a session's work and the next decision that is the operator's (13).

#### Scenario: A yes starts the work — mirrors S01
- **WHEN** the operator answers yes to a proposed elaboration
- **THEN** the elaboration is approved, its place is prepared, its session is
  charged, and nothing else is asked of the operator until the next decision
  that is theirs (13, 24)

#### Scenario: Approval is never re-asked
- **WHEN** a response has been applied
- **THEN** the same approval is never asked again, and a response that cannot be
  applied is reported rather than silently discarded (6)

### Requirement: The tail is derived from one mark per sink

The plan SHALL show, outside its count of decisions, what has reached done,
landed, closed or dropped since the last delivery to the sink the operator is
reading (14). One delivery mark per sink SHALL be the only recorded state behind
the tail, which SHALL be derived like the decisions (14).

#### Scenario: The tail is what happened since this sink last looked
- **WHEN** the operator opens the page after work finished and after the chat
  sink was last delivered to
- **THEN** the page's tail is measured from the page sink's own mark and the
  chat's from the chat sink's, and the two differ without either being stored as
  a rendering (14, 15)

#### Scenario: Delivery advances the mark in the same write
- **WHEN** a sink is delivered to
- **THEN** the decisions and the tail it carried and the advanced mark are one
  write, so a repeat of the delivery shows the same tail (14, 137)
