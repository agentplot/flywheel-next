## Purpose

The served page: one bundle that is the rail, the capture box and the status
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
  answer controls, the capture box, a capture's build, intent and drop
  controls, the dock's back control, and one instance of each kind's form
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

Until the instance's operators list holds more than one entry, a
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

The capture box in the header SHALL be the one place to type on the page, on the
desktop and as the phone's palette (S211). Text the operator types there SHALL be
a capture from the console with one signal of kind ask (19, S223). While that
signal has no move it SHALL stand on the rail as a numbered decision whose
answers are controls, never a word parsed out of the text (19a, 19, 194). The
page submission SHALL be the delivery, recorded once like any response (19).

#### Scenario: Typing into the capture box
- **WHEN** the operator types text into the capture box and submits it
- **THEN** one capture from the console exists with one signal of kind ask, the
  submission is recorded once as a response naming the capture it made, and the
  signal stands on the rail as a numbered decision with build, intent and drop
  as its controls (19, 19a, S223)

#### Scenario: A capture on the board
- **WHEN** a capture's signal is waiting on the operator
- **THEN** the board shows the capture as one card with its words from their
  beginning and its source, its whole face opening it in the dock, and its
  signal is not a second line (S215, S216)

### Requirement: The page is served at the address the host was given

The host's address SHALL be the hostname the operator gave, or localhost when
none was given, and never a name the machinery derived (204, 205a). The manifest
SHALL name the router per host (191). The host SHALL have one address with the
instance in the path, and every link SHALL be written at that address and name
the instance it opens, whether the address is a localhost port of the
operator's computer or a name on the operator's private network (205a, 308).
Served on the private network, the page SHALL work on a phone (155). Nothing
SHALL be published beyond the operator's private network unless the operator
says so (46).

#### Scenario: A link from a phone opens the object
- **WHEN** the host was given its name on the operator's private network and the
  operator taps a link carried by a chat rendering or a notification, from a
  phone on that network
- **THEN** the page opens that object in the dock with its answer controls in
  reach (308, 205a, 155)

#### Scenario: The host binds what it was given and no more
- **WHEN** the host is serving
- **THEN** it is bound to the address it was given and to a localhost port for
  the operator at the machine, to no other address, and the manifest names no
  publication (46, 155, 191, 245)

### Requirement: Every control answers at once, and a key is shown where it presses

A used control SHALL show within a frame that it was used, its form SHALL go busy
so a second press does nothing, and a bar SHALL run at the top of the page until
the page comes back (S210, 311). Escape SHALL close the topmost thing, in order:
the capture box, the field with the cursor, the dock, the log (S213). A key SHALL
be shown on the control it presses, the rail's head SHALL show the walk keys,
and a key a card does not offer SHALL be refused with the card's answers listed
(S218, S56, S57).

#### Scenario: A second press does nothing
- **WHEN** the operator presses an answer twice before its response lands
- **THEN** one response is recorded, and the control shows busy until the page
  comes back (S210, 137)

#### Scenario: A key the card does not offer
- **WHEN** the operator presses a letter the card in hand has no control for
- **THEN** nothing is answered and the card's answers are listed (S57, S218)

### Requirement: The rail and the board are one selection

One decision SHALL be in hand: the first at load, or the one whose object the
link opened (S219). The object it stands on SHALL be lit on the board and
scrolled into view, and where the board does not draw that object, its nearest
drawn parent SHALL be lit (S219). Walking the rail and taking a mark on a board
object SHALL move the hand (S219).

#### Scenario: A signal's decision lights its capture
- **WHEN** the operator walks the rail to a signal's decision
- **THEN** the capture the signal was read from is lit on the board and in view
  (S219)

### Requirement: A decision reads as a question

Each decision SHALL carry a sentence asking what it asks, with what the page
knows of its object, above controls that each say what pressing it does in a
verb, carry the key that presses it and one line of what follows; the value
posted SHALL be the model's own word (S220, 193, 194). The dock SHALL carry the
same sentence and controls in its head under the title, and SHALL have no foot
(S27).

#### Scenario: A bolt's close
- **WHEN** a bolt whose one unit has merged stands at its close on the rail
- **THEN** the card asks by name whether to land the bolt, its controls read
  `land it` and `hold` with their keys, and pressing `land it` posts the model's
  own answer (S220, 193)

### Requirement: The page keeps itself current

The host SHALL raise a generation when its store moved and tell every open page
over one event stream; the page SHALL fetch itself from the same host and swap
the regions that differ, keeping the decision in hand, the open dock and where
each column was scrolled (S221, 310). Every form SHALL be sent the same way, so
an answer or a capture never navigates (S221). No session SHALL call an address
to wake the loop, and the machinery SHALL build no wake API of its own: the loop
SHALL listen to the multiplexer's own events, so on a
host bound to Herdr an agent's change of state, and with it a session's report,
note, offer or refusal, SHALL be a cause at once; the poll SHALL stay the floor
(S221, 130).

#### Scenario: A session's report is a cause with no address called
- **WHEN** a session in a Herdr pane runs `flywheel exit done`
- **THEN** the loop takes the report up on the agent's change of state before
  the next poll, and neither the work order nor the session's environment
  carries an address of the page (S221, 130)

#### Scenario: A capture made elsewhere appears
- **WHEN** a capture is posted to the host while the page is open with a
  decision in hand
- **THEN** the capture appears on the open page without a reload, and the
  decision in hand and the lit object are kept (S221)

#### Scenario: Only the host is fetched
- **WHEN** the bundle is read for what it fetches
- **THEN** it fetches its own location and listens on the host's event stream,
  and names nothing external (310, S221)

### Requirement: The dock gives each kind its page

The dock SHALL give each kind its own page as `surfaces.md` S28 lists it, with no
record of keys and values and no sentence about the model (S28, S214). A bolt's
page SHALL show its repository, branch, host and place, every session on it and
the commits on its branch, and the page SHALL say *branch* for what the model
calls a line (S28, S222). A host SHALL be a chip in the hosts strip, and nothing
on the board or in the dock SHALL say "held by" (S216).

#### Scenario: A bolt's page
- **WHEN** the operator opens a bolt whose session delivered a commit
- **THEN** the dock shows its branch, host and place, the session with its
  agent, host, start, exit and delivery, and the commit on the branch, and the
  word *line* appears nowhere on it (S28, S222)
