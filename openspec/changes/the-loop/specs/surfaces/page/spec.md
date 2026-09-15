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
two kinds SHALL share one: a decision is the only answerable card, a proposal a
document with its unit proposals hanging off it, an intent a thread with its
elaborations in the order they were made, a bolt a ledger with its units in
order and their work items under them, a landed bolt a record, a capture a note
with its signals as quotes, a signal a quote, a session a row, and anything
else a plain entry (209). A part SHALL be drawn inside its whole when both sit
in one group, and where it sits otherwise; the phase an object is in SHALL be
shown by where it sits and never by its form (209). An elaboration SHALL be a
surface of its own, reached from its intent, and the intent's surface SHALL
list its elaborations in the order they were made, each with its type (210,
S28).

#### Scenario: A decision is the only answerable form
- **WHEN** the page renders a decision, a proposal, an intent, a bolt, a
  capture, a signal and a session together
- **THEN** the decision is the only thing shaped as an answerable card, and each
  other kind keeps its own form wherever it sits (209)

#### Scenario: A part hangs inside its whole
- **WHEN** a unit and its bolt sit in one group, and another unit of that bolt
  sits in another
- **THEN** the first is drawn inside the bolt's ledger, and the second is drawn
  where it sits (209)

#### Scenario: An elaboration opens from its intent
- **WHEN** the operator opens an intent
- **THEN** its elaborations are listed in the order they were made, each with
  its type and named by its type and the material it was proposed from (S226),
  and each opens its own surface, showing its type, where it stands, its intent,
  its decision when one is pending, its document and host, the intents it
  gathers when it covers several, and its sessions, or that none starts until it
  is approved (210, 188, S28)

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
a capture from the console with one signal of kind ask (19, S223). The capture
SHALL raise no decision; while its signal has no move it SHALL carry build now,
add to bolt…, make an intent, attach to… and drop as controls on the object,
never a word parsed out of the text, with one line under it saying who acts
next and when (19a, 19, 194, S224, S224a). The page submission SHALL be the
delivery, recorded once like any response (19).

At rest a capture SHALL show its words, its source and that line and no verb;
its five verbs SHALL show as one row under the line, in that order, while the
pointer or the focus is on it and while it is open in the dock, on the quote,
every tray row and a finding's row, and the drawer's footer SHALL carry them at
rest (S233). On a phone the quote and the tray row SHALL show no verb, and a tap
SHALL open the drawer with the verbs in its footer (S233, 311). While a capture
holds the focus, `b`, `a`, `m`, `t` and `d` SHALL press its verbs (S233, S218).
`attach to…` and `add to bolt…` SHALL open one picker inside the capture under
its verbs — above the drawer's footer, a bottom sheet on a phone — listing the
open intents or bolts with their count, a filter above eight rows, walked with
↓ and ↑ and picked with Enter, an empty list carrying the verb that makes one,
one picker open at a time and Esc closing it first (S233, S56, S57).

#### Scenario: Typing into the capture box
- **WHEN** the operator types text into the capture box and submits it
- **THEN** one capture from the console exists with one signal of kind ask, the
  submission is recorded once as a response naming the capture it made, the
  rail gains nothing, and the capture stands in Inception with the line saying
  curation reads it next and its five controls under the hand (19, 19a, S223,
  S224, S233)

#### Scenario: Twenty notes read as notes
- **WHEN** the signals tray lists twenty rows and the pointer rests on one
- **THEN** only that row shows its five verbs, and every other row is its words,
  its source and its line (S233)

#### Scenario: A picker inside a scrolling lane
- **WHEN** the operator presses `add to bolt…` on a capture in a scrolled lane
  with twelve bolts open
- **THEN** the capture grows to hold the picker under its verbs, nothing is
  clipped, a filter field takes the cursor, and Enter on a row adds the capture
  to that bolt and closes the picker (S233, S224a)

#### Scenario: A capture on the board
- **WHEN** a capture's signal is unmoved
- **THEN** the board shows the capture as one card with its words from their
  beginning and its source, its signal is not a second line, and a click
  anywhere on it opens it in the dock with its controls (S215, S216, S219,
  S224)

### Requirement: The capture box speaks the chat's grammar

The capture box SHALL carry the grammar the chat carries and no second one:
plain text a capture, a leading `/` a command of the catalogue, a bare number
the reply grammar (19, 193, 194). A leading `/` SHALL list the commands the
caller may invoke, matched as typed, each with what it does and what it acts on,
under a line saying exactly what will be sent; a command SHALL be the catalogue
tool of its own name, with what is typed after the name as its argument, and a
command that acts on an object SHALL take the one in hand — the object the dock
has open, else the decision the rail holds — or an id typed after the name
(S228). `/ask` SHALL take the repository as its first word and the rest as the
words, since nothing in hand names a repository (S228, 28). A command the palette cannot seed as one call SHALL be reached by its
own control and not listed (S228, 311). A bare number SHALL list that card's
answers, each with the line its control carries; a number with an answer SHALL
press the card's own control, a number with words SHALL send the words as the
answer, an answer that takes words SHALL be filled in for the operator to
finish, and a number naming nothing on the rail SHALL say so and say that
`/capture` keeps it as a note (S228, 194). `/` SHALL open the box with the
commands listed, and ↓ and ↑ SHALL walk the list while the box is open without
moving the rail (S56, S57).

#### Scenario: A command takes the object in hand
- **WHEN** the operator opens a unit in the dock, types `/drop` and presses
  Enter
- **THEN** the `drop` tool is called on that unit, recorded once like any
  response (S228, 193, 153)

#### Scenario: An ask typed in the box
- **WHEN** the operator types `/ask`, then `/ask storefront add a coupon field`
- **THEN** the box first says to type the repository, then the words, and then
  reads "will send: ask(storefront, …)"; Enter files the ask, recorded once
  like any response (S228, 193, 153)

#### Scenario: A number and its answers
- **WHEN** the operator types `412` in the box
- **THEN** the box lists card 412's answers with each control's line; `412 yes`
  presses the card's own control, `412: <words>` sends the words as the answer,
  and choosing `redo` fills in `412: redo: ` (S228, 194)

#### Scenario: A number naming nothing
- **WHEN** the operator types a number no decision on the rail holds
- **THEN** the box says so and says that `/capture` keeps it as a note (S228)

### Requirement: The curator's surface files a route's ask

Where the operator runs curation (93b), the page's curation section SHALL carry
each unmoved signal with the standing moves as controls, and picking route SHALL
swap the signal's target field for "ask in <repository> <words>": the
repository a chip when the instance tracks one and a picker when it tracks
several, the words starting as the signal's own (S229, 116). Submitting SHALL
file the ask through the `ask` tool, by the operator, and write the signal's
move as `route ask/<id>`; every ask SHALL be checked before anything is written,
so a refused one leaves no move and no exit, and a route that names nothing and
asks for nothing SHALL be refused (S229, 116, 93b). With no repository tracked
the field SHALL say so and say to add one to `flywheel.yaml` (S229). A signal
routed to an ask SHALL read "asked" under its capture's title, the capture's
dock page SHALL show the repository and the words, and Recently done SHALL list
it as asked (S229, S28, S9).

#### Scenario: A route becomes an ask
- **WHEN** the operator picks route for a signal on an instance tracking
  storefront and submits the words
- **THEN** `asks/storefront-1.rec` holds the words by the operator, the signal's
  move is `route ask/storefront-1`, and its capture reads asked (S229, 116)

#### Scenario: A refused ask writes nothing
- **WHEN** the operator submits the curation section with one ask the tool
  refuses among other moves
- **THEN** no move and no exit is written (S229)

### Requirement: The signals tray lists what curation has not read

The curation counter SHALL open the signals tray in the dock: every unmoved
signal grouped by capture and ordered by source and age, each row carrying its
capture's controls, with `run curation now` at its head beside the automatic
trigger stated in the operator's words (118, 19a, 110, S225). Pressing it SHALL
be the `curate` dictation (110, 193). While curation runs the tray SHALL show it
working and the count SHALL fall as moves are written; what curation proposes
SHALL land on the rail, one decision per proposal, and the tray SHALL ask
nothing itself (109, 116, S225). A finding a session offered with no intent or
bolt above it SHALL be a row like any other: its quote the document's path, its
source reading "offer", its line naming the session that offered it, with the
capture's five controls and no decision, and `build now` or `add to bolt…` on it
SHALL name its chore from the path's words (S231, S224a, 62).

#### Scenario: A session's offer waits in the tray
- **WHEN** a curation session offers a finding and no intent, bolt or capture
  stands above it
- **THEN** the tray lists one capture from that session, dated when the offer
  was made, with one row quoting the document's path under the source "offer",
  carrying the five controls, and the rail's count is unchanged (S231, 19a)

#### Scenario: Run curation now from the tray
- **WHEN** the operator opens the tray and presses run curation now
- **THEN** the curator session is charged at once, the tray's head shows it
  working, the count falls as moves are written, and each proposed intent or
  chore stands on the rail as one decision (110, 118, S225)

#### Scenario: Nothing waits
- **WHEN** no signal is unmoved
- **THEN** the tray says so in the operator's terms (S214, S225)

### Requirement: The page is served at the address the host was given

The host's address SHALL be the hostname the operator gave, or localhost when
none was given, and never a name the machinery derived (204, 205a). The manifest
SHALL name the router per host (191). The host SHALL have one address with the
instance in the path, and every link SHALL be written at that address and name
the instance it opens, whether the address is a localhost port of the
operator's computer or a name on the operator's private network (205a, 308). A
router base naming localhost with no port SHALL be the host's address at the
port its page serves on, so a link or a chat rendering is written there and
refused nowhere (245, 308, `host.yaml` address). Served on the private network,
the page SHALL work on a phone (155). Nothing
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
object SHALL move the hand (S219). A click anywhere on an object the board draws
SHALL open it in the dock and make it the selection, taking its decision in hand
where it has one and lighting it alone where it has none; nothing on the board
SHALL need its words or a link inside it clicked separately to open (S216,
S219).

#### Scenario: A click on a board object opens it
- **WHEN** the operator clicks anywhere on a bolt drawn on the board whose close
  waits on the rail
- **THEN** the dock opens on the bolt, the bolt is lit, and its decision is in
  hand on the rail (S219)

#### Scenario: A decision on an object the board does not draw
- **WHEN** the operator walks the rail to a decision whose object the board
  does not draw
- **THEN** the nearest parent the board draws is lit and in view (S219)

### Requirement: What finished lately is titled in plain words and windowed

The rail's list of what finished SHALL be titled in words that say what it holds,
"Recently done", and never "since" (S9). It SHALL hold today's entries, or the
last twenty when today holds fewer, and what falls off it SHALL stay on record
(S9). A capture SHALL enter it as captured, dated when the capture was put and
reading its words (S224, S9). A line for an object named by its id — a merged
or dropped unit, a landed bolt — SHALL carry its repository greyed before the
name, as a slip and a chores card's title do, and a capture or signal SHALL
read as its own words (S9, S14, S231).

#### Scenario: A note enters Recently done
- **WHEN** the operator types a note on the page
- **THEN** the newest entry under "Recently done" reads captured with the note's
  first words, dated when the capture was put (S224, S9)

#### Scenario: Two shared lines' chores read apart
- **WHEN** flywheel-next's `chore-1` and the blueprints' `chore-1` both merge
- **THEN** Recently done reads "flywheel-next · chore-1" and "blueprints ·
  chore-1" (S9)

#### Scenario: A quiet day
- **WHEN** twenty-five things finished yesterday and none today
- **THEN** the list under "Recently done" shows the latest twenty, and the other
  five are still on record (S9)

### Requirement: Proposed chores fold into one card, and a chore names its repository

Any fold of proposed chores, a bolt's or a repository's shared line's, SHALL be
one decision of kind chores, headed "<repository or bolt> · N chores", subtitled
with the session that offered them, its chores listed as lettered rows (11,
S231). A row SHALL be its letter, the document's name in words and what the
offer said it concerns, lettered over the batch's chores standing in the fold or
gone from it since the fold was raised, in the order their ids count them (S232).
Yes and drop SHALL answer every row still standing, and when the fold
holds more than one chore each row SHALL carry its own drop, since each row is
the chore's own decision; a row dropped first SHALL be out of the fold before
the yes, a letter SHALL stay with its chore, the fold SHALL keep its number
whichever row leaves, and no chore SHALL be answered by `pick` (11, 15, 60,
S232). The dock's page SHALL letter the rows the same way, each row's drop
reading `415b: drop` (S232). The
unit's page SHALL list every chore of the fold by its document and SHALL show a
bolt only when a bolt stands above it (S231, S28). A proposed chore on no ledger
SHALL be a slip naming its repository before its name, so two shared lines'
chores of one name read apart (S14). Accepted chores of a shared line SHALL show
in Construction as items on that repository's shared line under a chores head
per repository, and never as a bolt (S15, 60).

#### Scenario: Two repositories' chores fold apart
- **WHEN** sessions under no bolt offer two chores with `--scope storefront` and
  one with `--scope blueprints`
- **THEN** the rail carries one chores card headed storefront with a count of
  two and one headed blueprints with a count of one, each naming the session
  that offered it and answering yes or drop, and the storefront unit's page
  lists both documents and shows no bolt (S231)

#### Scenario: Some chores of a fold
- **WHEN** a storefront fold holds three chores and the operator presses drop on
  its first row, then yes on the card
- **THEN** the first chore is dropped, the second and third are accepted, and
  until the yes the card kept its number and the rows their letters b and c
  (15, S232)

#### Scenario: Slips read apart by repository
- **WHEN** storefront and the blueprints each hold a proposed `chore-1` on no
  ledger
- **THEN** Bolt plan shows two slips, each naming its repository before
  `chore-1` (S14)

#### Scenario: An accepted chore is not a bolt
- **WHEN** the operator says yes to a storefront chores card
- **THEN** Construction shows the chores as items under a chores head for
  storefront, and no ledger is drawn for them (S15, 60)

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
over one event stream; the page SHALL fetch from the same host what changed
since the generation it holds — the regions that moved, and the drawer's page
only while it is open — and swap those in, keeping the decision in hand, the
open dock and where each column was scrolled, and SHALL never fetch the whole
page again (S221, S235, 310). Every form SHALL be sent the same way, so
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
- **THEN** it fetches from its own host and listens on the host's event stream,
  and names nothing external (310, S221)

#### Scenario: One answer's update
- **WHEN** the operator answers a decision on an open page
- **THEN** what comes back is the regions that answer changed, at most 8 KB, and
  the page is not fetched again (S221, S235, 310a)

### Requirement: The page is built to a budget

The first view — the top, the rail, the board and the dock page a link named —
SHALL first paint within 1.0 s and take a press within 1.5 s on a mid-range phone
over an ordinary mobile connection, and a load with nothing cached SHALL be at
most 200 KB on the wire, 60 KB once the bundle's fonts, style and script are
cached, with the first view's HTML — what the host renders for that load, the
bundle's style and script out of it — at most 100 KB before compression (310a).
A load SHALL ship the first view
alone: a dock page SHALL be fetched when its object is opened, and the tray's
rows and every other list SHALL show 50 rows with their count and a `more`,
the rail never paged (310a, S235, 15). Every response SHALL be compressed. The
bundle's style, script and fonts are the same bytes for every load and every
instance, so each SHALL be a response of its own at the host's address, served
once and cached for a year under a name carrying the binary's version, and the
copy a member's client renders inside its own frame SHALL carry them inline,
since that frame fetches nothing (310a, S235, 291, 293a).
The time and the bytes SHALL grow with what is on screen and not with the
instance, and the page SHALL answer within its budget whatever the host is
doing: while the host's pass holds the store a read SHALL be answered from the
instance as it was last read, and a call that writes SHALL be taken as sent,
kept, made at the start of the host's next pass and recorded once, however many
times it was sent (310a, 137, 153). The numbers SHALL be held by a test measured
as the surfaces profile states (310a, S235).

#### Scenario: A phone opens the page
- **WHEN** the page is loaded in a browser throttled to a mid-range phone over an
  ordinary mobile connection, with nothing cached
- **THEN** it first paints within 1.0 s, its first card takes a press within
  1.5 s, and every response the first view needed sums to at most 200 KB
  (310a, S235)

#### Scenario: Ten times the instance
- **WHEN** the same first view is loaded on an instance holding ten times the
  signals, intents, captures and facts
- **THEN** it paints and weighs within a tenth of what it did (310a)

#### Scenario: A dock page is fetched when opened
- **WHEN** the operator opens a capture no link named
- **THEN** its page is fetched from the host, the drawer showing the capture's
  head at once and its body when it arrives, and the load carried no other dock
  page (S235)

#### Scenario: The host is recording a delivery
- **WHEN** the operator loads the page while the host records a curator's
  delivery
- **THEN** the page answers within its budget and does not wait for the pass
  (310a)

#### Scenario: An answer given while the host is busy
- **WHEN** the operator answers a decision while the host's pass holds the
  store, and presses the same control again before the pass ends
- **THEN** the answer is taken as sent at once, with the control spent until the
  next update draws it, the host makes it at the start of its next pass, and one
  response is recorded (310a, 137, 153)

### Requirement: A session chip's pane link says where the pane is

A session chip's pane link SHALL open a popover naming the multiplexer session
the pane is in, the host it runs on and the pane by its session id, with the
line that attaches a terminal to that session — naming the host's machine when
the host is another computer — and the line that focuses the pane, each with a
copy control (S234, 174, 196). A pane that is gone SHALL say so and offer
nothing to copy (S234). The page SHALL never open, focus or read a pane (68).

#### Scenario: Reaching a curator's pane
- **WHEN** the operator opens the pane link on a curator's chip
- **THEN** the popover names the machinery session, the host and the session id,
  and offers the attach line and the focus line to copy (S234)

#### Scenario: A pane already closed
- **WHEN** the operator opens the pane link of a session whose pane closed on its
  exit
- **THEN** the popover says the session exited and when, and offers nothing to
  copy (S234, 74)

### Requirement: The dock gives each kind its page

The dock SHALL give each kind its own page as `surfaces.md` S28 lists it, with no
record of keys and values and no sentence about the model, and a moment SHALL be
said as since when in the operator's own terms and never as a raw stamp (S28,
S214, 150a). A bolt's
page SHALL show its repository, branch, host and place, every session on it and
the commits on its branch, and the page SHALL say *branch* for what the model
calls a line (S28, S222). A host SHALL be a chip in the hosts strip, and nothing
on the board or in the dock SHALL say "held by" (S216).

#### Scenario: A bolt's page
- **WHEN** the operator opens a bolt whose session delivered a commit
- **THEN** the dock shows its branch, host and place, the session with its
  agent, host, start, exit and delivery, and the commit on the branch, and the
  word *line* appears nowhere on it (S28, S222)
