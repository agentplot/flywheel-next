## Purpose

The one write path into the flywheel: every operation the operator may invoke,
exposed as a tool with a schema naming its arguments by object id, called
identically by the page, the chat and the machinery.

## ADDED Requirements

### Requirement: Every operator operation is a tool, and no caller has one the others lack

Every operation the operator may invoke — capture, mark as intent, answer a
decision, drop, later, hold, release, rename, finish, end, close, retire,
takeover, revive, take, and the rest of what clause 4 grants — SHALL be exposed
by the state store as a tool with a schema naming its arguments by object id
(193). The page's controls, the chat and the machinery SHALL call the same
tools, and no caller SHALL have an operation the others lack (193).

#### Scenario: One catalogue, several callers
- **WHEN** the same operation is invoked from a page control and from the chat
- **THEN** both call the same tool with the same schema, and the record written
  is the same in either case (193)

#### Scenario: Every call is recorded once
- **WHEN** a tool is called
- **THEN** one response record is written, carrying the tool, the object, who
  gave it and when, and applied exactly once (153, 137)

### Requirement: A tool that asserts work was done does not exist

The operator MAY invoke any transition that undoes or defers work and SHALL NOT
invoke one that asserts work was done (4). No such tool SHALL exist, and a
response that arrives claiming one SHALL be recorded unapplicable and reported
under attention (4, 6). A session ended by hand SHALL be read as a session gone
and never as a response (4).

#### Scenario: An in-flight unit dropped, a claim of done refused — mirrors X08
- **WHEN** the operator drops an in-flight unit by dictation, then dictates that
  a work item is done, then kills a pane by hand
- **THEN** the unit is dropped with its sessions retired and its places
  released; the claim of done is refused, recorded unapplicable and reported;
  and the killed pane is read as a session gone, with a fresh attempt started
  rather than a response recorded (4, 12, 66, 74)

#### Scenario: An undo tool takes the decision's own transition
- **WHEN** the operator invokes an undo-or-defer tool outside a decision
- **THEN** the object takes the same transition the decision would have taken,
  with the same effects, and the act is recorded like any response (4, 12)

### Requirement: A dictation skips the plan and is applied directly

The operator's own dictation SHALL skip the plan and be applied directly (12).
A dictation SHALL name the object it acts on, and an approval it creates SHALL
be a response that can be pointed to (12, I1).

#### Scenario: The operator's own session — mirrors X01
- **WHEN** the operator opens a session of their own by dictation, on no thread
- **THEN** the session exists, no decision is ever raised about it, and it ends
  by dictation, its place going with it (69, 12)

#### Scenario: A dropped signal revived — mirrors S24
- **WHEN** the operator revives a dropped signal by dictation
- **THEN** the signal's move is replaced and the next curation run clusters it
  (107, 12)

### Requirement: The machinery never parses free text

Free text the operator types on the page or sends in chat SHALL NOT be parsed by
the machinery (194). The numbered reply grammar SHALL be the deterministic path
and SHALL itself be the answer tool (194). Text in the page's capture box SHALL
be a capture with one signal of kind ask, unparsed, and marking a capture as an
intent SHALL be a control and never a word read out of the text (19, 194).

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

### Requirement: The catalogue is one object, served over HTTP in this phase

The tool catalogue SHALL be one object with one definition per tool, and the
transport SHALL be a transport and never a second write path (193). This phase
SHALL serve it over HTTP for the page and call it in-process for the
machinery's own commands; further transports SHALL add clients and not
operations (193, 291, 293).

#### Scenario: A new transport adds no operation
- **WHEN** a further transport is added later
- **THEN** it serves the same catalogue, and no tool exists that the earlier
  transports lacked (193)

#### Scenario: The machinery's own commands go through the catalogue
- **WHEN** the machinery performs an operation the operator could also invoke
- **THEN** it calls the same tool function and the same record is written (193)
