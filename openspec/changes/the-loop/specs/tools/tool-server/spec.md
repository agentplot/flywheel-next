## Purpose

The one write path into the flywheel: every operation the operator may invoke,
exposed as a tool with a schema naming its arguments by object id, called
identically by the page, the chat and the machinery.

## ADDED Requirements

### Requirement: Every operator operation is a tool, and no caller has one the others lack

Every operation the operator may invoke — capture, propose a unit, open an
intent from a capture, attach or drop a signal, run curation, answer a
decision, ask, open-session, drop, later, hold, release, rename, finish, end,
close, retire, takeover, revive, take, and the rest of what clause 4 grants — SHALL be
exposed by the state store as a tool with a schema naming its arguments by
object id (193). The page's controls, the chat and the machinery SHALL call the
same tools, and no caller SHALL have an operation the others lack (193).

#### Scenario: One catalogue, several callers
- **WHEN** the same operation is invoked from a page control and from the chat
- **THEN** both call the same tool with the same schema, and the record written
  is the same in either case (193)

#### Scenario: Every call is recorded once
- **WHEN** a tool is called, from the page or from the chat
- **THEN** one response record is written, carrying the tool, the object, who
  gave it and when, applied exactly once, and the identity it names is the same
  operators-list entry either way (153, 137, 236a)

### Requirement: A tool that asserts work was done does not exist

The operator MAY invoke any transition that undoes or defers work and SHALL NOT
invoke one that asserts work was done (4). No such tool SHALL exist, and a
response that arrives claiming one SHALL be recorded unapplicable and reported
under attention (4, 6). A session ended by hand SHALL be read as a session gone
and never as a response (4).

#### Scenario: An in-flight unit dropped, a claim of done refused — mirrors X08
- **WHEN** the operator drops an in-flight unit by dictation, then dictates that
  a work item is done, then ends a session by hand outside the machinery's
  command so its presence evidence goes absent
- **THEN** the unit is dropped with its sessions retired and its places
  released, and the tail shows it dropped; the claim of done is refused,
  recorded unapplicable and reported; the ended session is read as a session
  gone and a fresh attempt is started rather than a response recorded; and
  nothing is merged (4, 12, 14, 66, 74)

#### Scenario: An undo tool takes the decision's own transition
- **WHEN** the operator invokes an undo-or-defer tool outside a decision
- **THEN** the object takes the same transition the decision would have taken,
  with the same effects, and the act is recorded like any response (4, 12)

### Requirement: A dictation skips the rail and is applied directly

The operator's own dictation SHALL skip the rail and be applied directly (12).
A dictation SHALL name the object it acts on, and an approval it creates SHALL
be a response that can be pointed to (12, I1).

#### Scenario: The operator's own session — mirrors X01
- **WHEN** the operator opens a session of their own by dictation, on no thread
- **THEN** the session exists and its record names who opened it, present means
  a keystroke within the window the profile states, no decision is ever raised
  about it, and it ends by dictation with its place going with it (69, 25, 12)

#### Scenario: An ask is a record for planning that holds its words
- **WHEN** the operator, or curation routing a signal that argues with no
  claim, calls `ask` naming a repository and the words
- **THEN** one ask record exists under `asks/`, written as the dictation's
  effect, holding the repository, the words, who gave it, when, and no
  consumer yet; its id is `<repository>-<n>`, the next number for that
  repository and never one used before; the call is recorded once as a
  response whose object is the ask, and the answer names it `ask/<id>`
  (28, 116, 153, git-only `layout.asks`, `surfaces.yaml` tools.ask)

#### Scenario: An ask naming no tracked repository or no words is refused
- **WHEN** `ask` is called naming a repository the instance does not track, or
  with no words
- **THEN** nothing is written, and the refusal names the tracked repositories
  where the repository was the reason (`surfaces.yaml` tools.ask)

#### Scenario: A curation session files an ask through its own command
- **WHEN** a curation session runs `flywheel ask <repository> <words>` in its
  place for a signal that argues with no claim
- **THEN** the `ask` tool is called with the session's identity, the ask record
  written is the one the operator's dictation writes with the session as who
  gave it, nothing is written on the session's thread, `ask/<id>` alone is
  printed, and the signal's route move names it (67, 116, 197, `sessions.yaml`
  commands.ask)

#### Scenario: An ask from a session not granted it is refused
- **WHEN** a session other than curation or the operator's own session runs
  `flywheel ask`, or any session names a repository the instance does not track
- **THEN** no ask record is written, the refusal is an entry on the session's
  own thread naming `ask` as the operation, or naming the repository and the
  tracked ones where the repository was the reason, and the command exits 1
  (43, 197, `sessions.yaml` commands.ask)

#### Scenario: A granted session's work order carries the command
- **WHEN** a work order is rendered for a curation session or the operator's
  own session
- **THEN** under how to report it carries the exact `flywheel ask` command with
  the state and the manifest filled in (67, `sessions.yaml` commands.ask)

#### Scenario: A dropped signal revived — mirrors S24
- **WHEN** the operator revives a dropped signal by dictation
- **THEN** the drop move is removed, the signal is unmoved again, no decision is
  raised for it, and the next curation run clusters it (107, 12)

### Requirement: The machinery never parses free text

Free text the operator types on the page or sends in chat SHALL NOT be parsed by
the machinery (194). The numbered reply grammar SHALL be the deterministic path
and SHALL itself be the answer tool (194). Plain text in the page's capture box
SHALL be a capture with one signal of kind ask, unparsed, and marking a capture
as an intent SHALL be a control and never a word read out of the text (19, 194).
The box's leading `/` SHALL name a command of the catalogue and resolve to that
one tool call, and its bare number SHALL be the reply grammar, the same grammar
the chat carries (19, 193, 194, S228).

#### Scenario: A capture is not read for commands
- **WHEN** the operator types text beginning with a word that looks like a
  command into the capture box
- **THEN** the whole text is captured verbatim as one signal of kind ask, and no
  part of it is interpreted (19, 194)

#### Scenario: A message that is neither a numbered reply nor a forward
- **WHEN** free text arrives in chat that is not the reply grammar and not a
  forward
- **THEN** nothing is written and the operator is answered with what the sink
  accepts, because the machinery does not parse it (194)

### Requirement: The catalogue is one object, whatever transport reaches it

The tool catalogue SHALL be one object with one definition per tool, and the
transport SHALL be a transport and never a second write path (193). This phase
SHALL serve it over HTTP for the page, call it in-process for the machinery's
own commands, and serve it at the host's address as a remote server of the
model context protocol for a member's client (293, 293a, 320); a further
transport SHALL add clients and not operations (193; proposal, What must not be
foreclosed).

#### Scenario: Every caller sees one catalogue
- **WHEN** the in-process caller, the HTTP caller and a caller over the
  protocol each enumerate the catalogue
- **THEN** they list the same tools with the same schemas, so a transport adds a
  client and not an operation (193, 293; proposal, What must not be foreclosed)

#### Scenario: The machinery's own commands go through the catalogue
- **WHEN** the machinery performs an operation the operator could also invoke
- **THEN** it calls the same tool function and the same record is written (193)

### Requirement: A member's client reaches the catalogue at the page's address

The protocol SHALL be answered at the address the page is served at, with the
instance in the path: one message a request posted to `/<instance>`, with no
session to hold (205a, 291, 319). The host SHALL open no stream to a client, a
client asking for a stream alone SHALL be told the host offers none, and a
message naming another instance SHALL be refused (310, 325, 205a). A call MAY
name its own delivery under `_meta` as `flywheel/delivery` — letters, digits,
`.`, `_` and `-`, at most 64, never a bare number — and SHALL be recorded under
that name as `client-<name>`, so the same call delivered twice is one response;
a call naming none SHALL be counted `client-<n>`; the record's delivery SHALL be
`client` (137, 323). A call the catalogue refuses, a view refused for want of
its object and a caller refused at the door SHALL each be one run-record entry
of kind `refusal` naming the identity, `unsigned-in` at the door, the operation,
the object the call named — a decision by its number, `none named` when none was
— and delivery `client` (79, 321, 253a). At start the host SHALL print
`client at <address> · <authority>` once per address it listens on; in this
phase the authority is 253a's exception, naming the one operators-list entry
every call is given by, and with more than one entry saying no client is served
(320, 253). A client SHALL be a caller and never a host: a call over the
protocol runs no tick and takes no lease, and no credential of a client is
written to the state store, the manifest, a record or the page (325, 291, 204,
207).

#### Scenario: A client asks for a stream
- **WHEN** a client sends a GET to `/<instance>` asking only for an event stream
- **THEN** the host answers that it offers none and opens no stream (310, 325)

#### Scenario: The same call delivered twice
- **WHEN** a client sends the same `answer` call twice naming one delivery
- **THEN** one response `client-<name>` is recorded with delivery `client`, and
  the second delivery writes nothing (137, 323)

#### Scenario: A refusal at the door reaches the run record
- **WHEN** a caller the host does not admit calls a tool over the protocol
- **THEN** the run record holds one `refusal` entry with identity
  `unsigned-in`, the operation, the object and delivery `client`, and no
  response is recorded (79, 321, 253a)

#### Scenario: The host says where to add a client
- **WHEN** a host with one operators-list entry starts
- **THEN** it prints `client at http://<listener>/<instance>` beside the
  authority, once per address it listens on, naming the entry every call is
  given by (320, 253a)

### Requirement: A member's client renders the page's own views

The views a member's client renders SHALL be the page's own — the rail, the
board, the status view and one object's detail — carried under the model
context protocol's user-interface extension (293a, 322, S230). Each SHALL be
the result of a read-only query named for it — `rail`, `board`, `status`,
`object <object>` — that writes and records nothing and follows the catalogue's
tools in every enumeration (193, 322). A result SHALL carry the view's regions
keyed by their element ids on the page — the rail; the board's header and four
lanes; for `status` the hosts strip with them; for `object` its dock surface,
opened — and the same view in words (141, 311). A tool whose result is a view
SHALL name the view's resource on its declaration (`_meta.ui.resourceUri`) and
never on its result. The address SHALL be
`ui://flywheel/<version>/<rail|board|status|object>`, read under the caller's
identity like any call, and every address SHALL answer the one bundle, the
page's template with nothing of the state drawn, with media type
`text/html;profile=mcp-app`, drawing the regions and the state the tool returned
(293, 293a, 307, 310); inside a client only the view's regions SHALL show, at
the frame's width. An address under another version SHALL be refused naming the
served ones. Every result SHALL carry the version it was rendered under, and a
view whose bundle is of another version than its result SHALL empty every
region and show that it is out of date (326). The bundle SHALL declare no
external origin and ask no permission of the client's sandbox (307, 310, 204).
The four views SHALL be listed among the server's resources. A tap inside a
rendered view SHALL be a `tools/call` through the client on the tool and object
the page's form would post — `yes all` one `answer` per number, in order —
checked, recorded once and idempotent as any call; the control SHALL go busy at
once, the view SHALL fetch itself again once the calls are made, and a refusal
SHALL show as a toast with its reason (321, 323, 137, S7, S30). A link to an
object SHALL open that object's view through the client, and a link out of the
flywheel SHALL be handed to the client (308, 315). A client that renders none of
them SHALL still hold every tool, and every decision SHALL stay answerable as a
call (311, 322).

#### Scenario: A view is named where the tool is declared
- **WHEN** a client lists the tools and calls one whose result is the rail
- **THEN** the tool's declaration names `ui://flywheel/<version>/rail`, the
  result names no resource and carries the version, and reading the address
  returns the page's bundle with the extension's media type (S230, 307)

#### Scenario: A copy of another version is not rendered as state
- **WHEN** a client holding the bundle of an earlier binary renders a result a
  newer binary returned
- **THEN** the view empties every region and shows "this view is out of date ·
  fetch it again", and the newer binary's declarations name the newer address
  (326, S230)

#### Scenario: Yes all through a client
- **WHEN** the operator taps `yes all` naming three decisions inside a rendered
  rail
- **THEN** the client sends three `answer` calls in order, each recorded once,
  and the view fetches itself again and redraws (323, S7)

#### Scenario: A tap is a call through the client
- **WHEN** the operator taps yes on a decision inside a rendered rail
- **THEN** the client sends `tools/call` on the answer tool for that decision,
  the response is recorded once with who gave it and when, and a second
  delivery of the same call writes nothing (323, 321, 137, 153)

#### Scenario: A client that renders nothing
- **WHEN** a client that renders no resource enumerates the catalogue and calls
  `rail`
- **THEN** the result's words give every decision's number and answers, and
  every decision is answerable by a call it holds (311, 322)
