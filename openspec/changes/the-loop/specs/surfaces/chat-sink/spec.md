## Purpose

The chat the operator already reads: one line per decision with the same number
the page shows, a link back to the object, the numbered reply grammar, and the
notification that reaches a phone.

## ADDED Requirements

### Requirement: The chat carries the same decisions and numbers as the page

A chat rendering of the plan SHALL carry the same decisions and numbers as the
page, one line each, and a link to the page (18). It SHALL keep each kind's form
in one line, so that a decision line is answerable and no other line is (18,
209).

#### Scenario: One line per decision, same numbers
- **WHEN** the plan is delivered to the chat sink and to the page
- **THEN** each decision appears once in the chat as one line carrying the same
  number the page shows, with a link to that object on the page (18, 15)

#### Scenario: Only a decision line is answerable
- **WHEN** the chat rendering carries a decision, a tail entry and a signal
- **THEN** the decision line offers an answer and the others do not (18, 209)

### Requirement: A short reply from a phone is sufficient for any decision

The response MAY be given as a short reply in a chat channel or as a choice on a
served page, and either SHALL be sufficient for any decision (2, 152). The
numbered reply grammar SHALL always work beside whatever controls the platform
provides (309, 194). A response SHALL be recorded with the object, the decision,
who gave it and when, before any work follows from it (153), and the operator
SHALL be able to tell it was recorded (154).

#### Scenario: Answering by number from a phone
- **WHEN** the operator replies with the reply grammar naming a decision number
- **THEN** the response is attributed to that decision, recorded before anything
  follows, and acknowledged so the operator can tell it landed (2, 152, 153,
  154)

#### Scenario: Platform controls and the grammar stand together
- **WHEN** the platform offers a control for answering
- **THEN** it is used as the platform provides it, and the numbered grammar
  still works beside it (155, 309)

### Requirement: The chat accepts two shapes of message and refuses to guess at the rest

The chat sink SHALL accept the numbered reply grammar, which is the answer tool,
and a forwarded message, which is a capture (194, 112, 215). Any other message
SHALL be answered with one reply saying what the sink accepts and SHALL write
nothing, because the machinery never parses free text (194).

#### Scenario: A forwarded message becomes a capture — mirrors S21
- **WHEN** the operator forwards a chat message with one response
- **THEN** one capture and one signal exist with a link back to the source, and
  nothing else happens (112, 215)

#### Scenario: Free text writes nothing
- **WHEN** a message arrives that is neither the reply grammar nor a forward
- **THEN** the sink replies with what it accepts, writes no record, and
  interprets nothing (194)

### Requirement: A decision reaches the phone through the chat sink's notification

A decision raised SHALL reach the operator's phone through the chat sink's own
notification, carrying the answer controls the platform provides and a link to
the object on the page (309). The page SHALL send no push of its own (309).
Every chat rendering, notification and plan line SHALL carry a link at the
host's address with the organization in the path, opening that object in the
dock with its answer controls in reach; a link to a host that is away SHALL say
so rather than failing silently (308, 205a, 150a).

#### Scenario: A new decision reaches the phone
- **WHEN** a decision is numbered and routed to the chat sink
- **THEN** the notification the platform raises carries the number, whatever
  controls the platform provides, and the link to the object (309, 308)

#### Scenario: A link to a host that is away
- **WHEN** the operator opens a link to an object held by a host past its stale
  window
- **THEN** the page says the host is away and since when, rather than failing
  silently (308, 150a)

### Requirement: One presenter per sink, one mark per sink

Exactly one presenter SHALL deliver to each sink at a time, held by lease or
pinned by the manifest (148). Each sink SHALL carry its own delivery mark, so
the tail since the last look is that sink's (14, 148, 236). Sinks SHALL be per
member, and a shared channel SHALL be a sink of its own with one mark (236).

#### Scenario: One delivery, not two
- **WHEN** more than one host could present the chat sink
- **THEN** the holder of that sink's lease delivers and the others do not, so
  the operator sees each delivery once (148)

#### Scenario: Each sink's tail is its own
- **WHEN** the chat sink and the page sink have different marks
- **THEN** each delivery carries the tail since that sink's own mark (14, 236)

### Requirement: Notifications are routed by kind to the sinks the operator sets

Notifications SHALL be routed by kind — a blocked session, a stalled session, a
lost host, a failed landing, new decisions — to sinks the operator sets per kind
(82). A host running construction SHALL be silent by default, and nothing the
machinery notices SHALL be visible only on the host that noticed it (82).

#### Scenario: A kind routed to the chat and the page
- **WHEN** the operator routes a kind to both sinks and an event of that kind
  occurs on a host that presents neither
- **THEN** both sinks carry it, and the noticing host shows it nowhere of its
  own (82)

#### Scenario: A kind the operator routed nowhere
- **WHEN** an event of a kind occurs whose routing names no sink
- **THEN** it is still recorded in the run record and is reachable from the
  status view, so nothing is lost (79, 82)
