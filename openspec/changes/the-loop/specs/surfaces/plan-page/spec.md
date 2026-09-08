## Purpose

The served page: one bundle that is the plan, the capture box and the status
view, answerable on a phone under 760px and on the desktop from the same build,
on the operator's private network.

## ADDED Requirements

### Requirement: Desktop and phone are one bundle

Every decision SHALL be answerable on a phone, and every control and every form
the page carries SHALL be available there; nothing the page offers SHALL be
reachable only on a desktop and nothing the phone answers SHALL be missing on
the desktop (306, 2, 155). The phone SHALL be the same served bundle under
760px and never a second application: two tabs, Decisions and Board, with the
dock full screen and a back control (307). One bundle SHALL be built, one
served, and its version SHALL be the binary's (307).

#### Scenario: Approving from the phone at 390px — mirrors S01
- **WHEN** the operator opens the page at a 390px viewport and answers a
  proposed elaboration
- **THEN** the decision's number and answers are visible and answerable there,
  the response is recorded, and the work starts (306, 314, 1, 13)

#### Scenario: Nothing is desktop-only
- **WHEN** any control or form the page carries is compared between the desktop
  and the 390px viewport
- **THEN** each is reachable in both, with the phone's layout the same bundle
  under 760px (306, 307)

### Requirement: Every answer is one tap or one short reply

Every answer SHALL be one tap or one short reply (311). Nothing SHALL be
reachable only by hover or by a keyboard, and anything a hover reveals on the
desktop SHALL be reachable by tap on a phone (311). A long-form answer SHALL be
given on the phone with the platform's own keyboard (311).

#### Scenario: No hover-only affordance
- **WHEN** the page is driven by taps alone at a 390px viewport
- **THEN** every decision can be answered and every disclosure opened, with no
  affordance requiring a pointer or a key (311)

### Requirement: The status view renders from one request and holds no client state

The status view SHALL render from one request (310). The page SHALL hold no
client state a reload loses, so a reload after an answer SHALL show the answer
recorded with who gave it and when (310, 153, 154). The bundle SHALL carry no
dependency the phone must fetch from anywhere else (310).

#### Scenario: A reload after an answer
- **WHEN** the operator answers a decision and reloads the page
- **THEN** the answer is shown as recorded, with who gave it and when, and
  nothing the operator had entered is lost to the reload (310, 153, 154)

#### Scenario: The bundle fetches nothing external
- **WHEN** the page loads with no route beyond the operator's private network
- **THEN** it renders whole, fetching no script, font or style from anywhere
  else (310)

### Requirement: The page shows the whole and gives each kind one form

The page SHALL show every intent, elaboration, bolt, unit, work item and session
with its current state, grouped by state, and for each which host holds it, which
runs it and whether that host is alive (141). Every kind of object SHALL have one
form of its own and no two kinds SHALL share one; the phase an object is in SHALL
be shown by where it sits and never by its form (209). An elaboration SHALL be a
surface of its own, reached from its intent, and the intent's surface SHALL list
its elaborations in order (210).

#### Scenario: A decision is the only answerable form
- **WHEN** the page renders a decision, a proposal, an intent, a bolt and a
  signal together
- **THEN** the decision is the only thing shaped as an answerable card, and each
  other kind keeps its own form wherever it sits (209)

#### Scenario: An elaboration opens from its intent
- **WHEN** the operator opens an intent
- **THEN** its elaborations are listed in order and each opens its own surface,
  showing its type, its state, its decision when one is pending, and its
  session's last activity (210)

### Requirement: The page is a capture surface

Text the operator types on the page SHALL be a capture with one signal of kind
ask, so curation sees it (19). The operator MAY mark a capture as an intent, and
that SHALL be a judgment made with a control and never a word parsed out of the
text (19). The page submission SHALL be the delivery, recorded once like any
response (19).

#### Scenario: Typing into the capture box
- **WHEN** the operator types text into the capture box and submits it
- **THEN** one capture exists with one signal of kind ask, and the submission is
  recorded once as a response naming the capture it made (19)

#### Scenario: Marking a capture as an intent
- **WHEN** the operator uses the mark-as-intent control on a capture
- **THEN** an intent is opened by that judgment, and no part of the capture's
  text decided it (19)

### Requirement: The page is served at the host's private-network address

The page SHALL be served on the operator's private network and SHALL work on a
phone (155). The host SHALL have one address with the organization in the path,
and a link SHALL name the organization it opens (205a). Nothing SHALL be
published beyond the operator's private network unless the operator says so (46).

#### Scenario: A link from a phone opens the object
- **WHEN** the operator taps a link carried by a chat rendering or a
  notification, from a phone on the operator's private network
- **THEN** the page opens that object in the dock with its answer controls in
  reach (308, 205a, 155)

#### Scenario: Nothing is published without the operator's word
- **WHEN** the page is served
- **THEN** it is reachable on the operator's private network and at no address
  beyond it, unless the operator has chosen to publish one (46, 155)
