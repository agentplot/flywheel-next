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
- **WHEN** the driver enumerates a fixed list at both viewports — a decision's
  answer controls, the capture box, the mark-as-intent control, the dock's back
  control, and one instance of each kind's form
- **THEN** every one is reachable at both, with the phone's layout the same
  bundle under 760px (306, 307)

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

### Requirement: The page carries the status view and gives each kind one form

The page SHALL carry the status view, whose contents are stated in the run
record's spec (141). Every kind of object SHALL have one form of its own and no
two kinds SHALL share one; the phase an object is in SHALL be shown by where it
sits and never by its form (209). An elaboration SHALL be a surface of its own,
reached from its intent, and the intent's surface SHALL list its elaborations in
order (210).

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

### Requirement: A single-operator host on a private network serves the page unsigned-in, and every response names that operator

Until the organization's operators list holds more than one entry, a
self-managed host MAY serve the page on the operator's private network with no
sign-in (253a). The single entry SHALL be the identity every response records as
given by, with when (253a, 153, 236a). The host SHALL refuse to serve
unsigned-in as soon as a second operator is listed or the page is reached at any
address but that network's (253a). The exception SHALL close when the account
item exists (253a, 233).

#### Scenario: A response names the manifest's operator
- **WHEN** the operator answers a decision on the unsigned-in page
- **THEN** the response record's given-by field is the operators list's single
  entry and its given-at is set (153, 236a, 253a)

#### Scenario: A second operator closes the exception
- **WHEN** a second entry is added to the operators list and the page is
  requested unsigned-in
- **THEN** the host refuses to serve it and says why (253a)

#### Scenario: An address off the private network is refused
- **WHEN** the page is requested at an address that is not the host's
  private-network address
- **THEN** the host refuses to serve it unsigned-in (253a)

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
phone (155). The manifest SHALL name the router per host, and this phase's host
address SHALL be the private-network router's name for the host (191, 205a). The
host SHALL have one address with the organization in the path, a link SHALL name
the organization it opens, and a link SHALL never name a localhost port (205a,
308). Nothing SHALL be published beyond the operator's private network unless
the operator says so (46).

#### Scenario: A link from a phone opens the object
- **WHEN** the operator taps a link carried by a chat rendering or a
  notification, from a phone on the operator's private network
- **THEN** the page opens that object in the dock with its answer controls in
  reach (308, 205a, 155)

#### Scenario: The host binds two addresses and no more
- **WHEN** the host is serving
- **THEN** it is bound to its private-network address and to a localhost port
  for the operator at the machine, to no other address, and the manifest names
  no publication (46, 155, 191, 245)
