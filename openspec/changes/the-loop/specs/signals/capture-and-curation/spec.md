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

### Requirement: This phase ships five adapters, all of them enumerators

The flywheel SHALL ship adapters that write one keyed capture per source event
with a pointer to the raw material (215). This phase SHALL ship five: the
page's capture box (19), the chat forward (112, 215), the meeting transcript
(111, 215), the signals folder of captures already read (114, 215) and the
folder drop (215). Enumerating source events and writing captures SHALL run
unattended; turning a capture into signals SHALL be a judgment and SHALL NOT
run unattended (115), except where the capture is its own excerpt (19, S21) or
its signals were written before the instance existed (114).

#### Scenario: A signals folder is read as captures already read
- **WHEN** a directory holding one directory per capture, each with a
  `capture.md` provenance header and one markdown file per signal, is named to
  the signals-folder adapter
- **THEN** each becomes a capture record with one signal record per file,
  keeping the capture's source and event date and each signal's kind, excerpt
  and assertion as they were read, no reader is charged, and naming the
  directory again writes nothing (114, 111, 217e)

#### Scenario: A move by a word the flywheel does not ship
- **WHEN** the folder's `moves.rec` records a move whose word is not one of
  attach, challenge, join, answered, route or drop
- **THEN** that signal arrives unmoved and curation judges it (114, 107, 118)

#### Scenario: A dropped file waits for its reader
- **WHEN** a file is dropped in a folder the host declares as a source
- **THEN** one capture with a pointer and no signals is written, and one reader
  session is charged by the tick of the declaring host (215, 115, 217e)

#### Scenario: An enumerator writes a capture and starts nothing
- **WHEN** a source event arrives — a transcript named to the capture command, a
  message forwarded to the chat sink, or a submission of the page's box
- **THEN** one keyed capture is written, no session is started, and no judgment
  about the material is made (115, 215)

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
(113). The signal and move record formats SHALL be versioned and stable (114).
Captures made before the instance existed SHALL be read without a reader's
judgment, an adapter carrying their form into the record format (114).

#### Scenario: A signal is not edited
- **WHEN** the tool catalogue is enumerated and a signal's history is read
- **THEN** no tool edits a signal, and the signal's record is never rewritten in
  history; a correction is a new signal or a change of move (113, 193)

#### Scenario: An older record reads without conversion
- **WHEN** a capture or signal written under an earlier version of the format is
  read
- **THEN** it is read as it stands, with no migration step (114)

### Requirement: Every signal has exactly one standing move

Every signal SHALL have exactly one standing move — attach, challenge, join,
answered, route or drop — stored with the signal id, the target, the reason and
the date (107). Curation SHALL run over signals with no move and SHALL NOT
re-judge one that has a move; only the operator's response SHALL replace a move
(107). Every move SHALL have a stated consequence (116). A route move SHALL name
what curation, being a session, offered for a signal that argues with no claim:
a chore through `flywheel offer --scope <repository>`, which `record_offers`
makes a proposed chore unit under the repository the scope names, on its shared
line (60, 62), or an ask filed by running `flywheel ask <repository> <words>` in its
place, which calls the `ask` tool as the session, writes the ask record and
prints the id the route names (116, 58–60, 28, 67). A route naming an offer
entry SHALL be written on the signal as the unit that offer became (116). An
exit SHALL NOT carry an
ask, and a route that offers neither a chore nor an ask SHALL be refused (116).
A challenge move SHALL record
the claim it argues with by name and version; the consequence that stales that
claim's verdicts (101) belongs to the phase that has a ledger.

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
- **THEN** the drop move is removed, the signal is unmoved again, and the next
  curation run clusters it (107)

### Requirement: Every offer is recorded on the pass that finds it

Every offer on a session's thread SHALL become one record pointing at its
document on the pass that finds it, so no offer holds a session from its exit
(62). A finding offered under neither an intent nor a bolt SHALL be a signal:
the next signal of the capture the session was reading when there is one, and
otherwise the one signal of a capture of its own — source `offer`, keyed
`offer/<session>/<entry>`, captured by the session at the moment of the offer,
the document its raw material — both written into the blueprints' signals as a
page capture's are (62, 111, 113, S231). That signal SHALL be of kind ask,
asserted by the session, its assertion the document's path, its excerpt empty
and its position whole, and SHALL stay unmoved until curation or the operator
moves it (62, 19a, 107). A chore offered off every bolt SHALL NOT be a signal:
it SHALL be a proposed chore unit on the shared line of the repository its
offer names, folded on the rail with that repository's other proposed
shared-line chores into one decision headed by its name and answered yes or
drop, as a bolt's chores fold by bolt (60, 62, 11, S231). The offer SHALL say
where a chore's fix belongs through `--scope`: `bolt-line` under a bolt, which
MAY be left off, and otherwise a tracked repository's manifest name or
`blueprints`, the names `propose-chore` and `flywheel ask` take (60, 123); under
a bolt a repository's name SHALL put the chore on that shared line and not on
the bolt. A chore unit SHALL record its scope as `bolt-line` or `shared-line`
(60; `unit.yaml`). A chore of the blueprints SHALL stand under the instance
with repository `blueprints` and fold by that name (123). A unit under a
repository or under the instance SHALL work off that shared line, and a yes
SHALL start its one session in a place off it, merged there (60). A chore
offered under no bolt whose scope names no tracked repository SHALL be refused
on the session's thread with the tracked names and `blueprints`, exit 1 and
never be pending, as an ask is refused; one that reached the thread without
that check SHALL be refused there by `record_offers` with `refuses: <entry>`,
so it holds no session and nothing is made of it (60, 62; sessions.yaml
`commands.offer`, `record-derived.yaml`). The command SHALL find the session's
object from the session id alone, as the longest prefix of the id on record,
and nothing SHALL be read from `--about` (sessions.yaml `commands.offer`). An
offer of kind signal, which a session makes when what it saw is about neither
its intent nor its bolt, SHALL be recorded by the path a finding with nothing
above the session takes, wherever the session stands, and SHALL NOT be a
proposal on its thread (58, 62). Every offer's entry SHALL name the place's
revision at the offer, and an offer whose document that revision does not hold
SHALL be refused on the session's thread naming the path, exit 1 and never be
pending; a unit made of a finding or a chore SHALL keep the document's path and
that revision (62; sessions.yaml `commands.offer`, `unit.yaml` record). A
chore's session SHALL be handed the document as its job in its work order, read
at the yes from the offering session's repository at that revision (62, 89;
chore@2 `params.job`, host.yaml `prepare_place`). A chore whose offering place
was removed without merging before the yes SHALL be withdrawn (62).

#### Scenario: A curation session's finding reaches its exit
- **WHEN** a curation session offers a finding and exits done
- **THEN** a capture of source `offer` holds one signal of kind ask whose
  assertion is the document's path and whose excerpt is empty, the signal is
  unmoved, and the session reaches its exit (62, 113, S231)

#### Scenario: A capture reader's finding is its capture's next signal
- **WHEN** a capture reader offers a finding about the capture it reads
- **THEN** the finding is that capture's next signal and no other capture is
  written (62, 113)

#### Scenario: A chore off every bolt is its repository's
- **WHEN** a session under no bolt offers a chore with `--scope` naming a
  tracked repository
- **THEN** a proposed chore unit under that repository points at the document,
  no capture or signal is written, and the rail shows it in one decision headed
  by the repository's name with its other proposed shared-line chores (60, 62,
  S231)

#### Scenario: A signal offered under a bolt is a signal
- **WHEN** a session working under a bolt offers a signal
- **THEN** a signal is written as for a finding with nothing above the session,
  no unit is made on the bolt, and the session reaches its exit (58, 62)

#### Scenario: An offer of a document not committed is refused
- **WHEN** a session offers a document its place's head does not hold
- **THEN** the command exits 1, the refusal on the session's thread names the
  path, and nothing is pending (62)

#### Scenario: A chore's session reads the chore in its order
- **WHEN** the operator says yes to a flywheel-next chore a curation session
  offered from its place on the blueprints
- **THEN** the chore's work order carries the document's text as it stood at the
  offer's revision, and the session on flywheel-next's shared line needs nothing
  from the curation session's place (62, 89)

#### Scenario: A chore offered from a place removed unmerged is withdrawn
- **WHEN** the place a chore was offered from is removed without merging before
  the operator answers the chore
- **THEN** the chore is withdrawn and its decision leaves the rail (62)

#### Scenario: A chore of the blueprints stands under the instance
- **WHEN** a session under no bolt offers a chore with `--scope blueprints`
- **THEN** the proposed chore unit stands under the instance with repository
  `blueprints` and folds on the rail under that name (60, 123)

#### Scenario: A chore off every bolt that names no tracked repository
- **WHEN** a session under no bolt offers a chore with no scope, or with a scope
  the instance does not track
- **THEN** the offer is refused on the session's thread with the tracked names
  and `blueprints`, the command exits 1, and nothing is pending or written (60)

#### Scenario: A chore the command never checked is refused on its thread
- **WHEN** a chore entry off every bolt whose scope names no tracked repository
  stands on a session's thread without the command having refused it
- **THEN** `record_offers` writes `refuses: <entry>` on that thread, the offer is
  not pending, no session is held on it, and no unit, capture or signal is
  written (60, 62)

#### Scenario: Under a bolt, a repository's scope is its shared line
- **WHEN** a session under a bolt offers a chore with `--scope storefront`
- **THEN** the proposed chore unit stands under the storefront repository with
  scope `shared-line`, and nothing is added to the bolt (60)

#### Scenario: A route naming an offer names its unit
- **WHEN** curation routes a signal naming the chore entry it offered with
  `--scope storefront`
- **THEN** the signal's move is a route naming the proposed chore unit that
  offer became (116, 62)

### Requirement: A capture is a note, and the operator may move its signal by hand

A signal with no move SHALL NOT be a decision, and nothing on the rail SHALL ask
the operator what a capture should become (19a). While its signal is unmoved,
the capture SHALL carry the operator's controls on the object: build now
(`propose-unit`), make an intent (`open-intent`), attach to an open intent
(`attach-signal`) and drop (`drop-signal`), each a dictation that writes the
move curation would have written and is the signal's one move (19a, 107, 110,
116, 193). Build now SHALL name the bolt from the capture's own words and ask
for no name (19a, S217). Make an intent SHALL open the intent at once, the
gesture being its approval, with the signal attached (12, 19a). Under the
capture one line SHALL say who acts next and when, read from the curation
record's unmoved count, threshold and cadence (19a, 110, 118).

#### Scenario: A capture lands with nothing to answer
- **WHEN** the operator types a note on the page
- **THEN** a capture with one unmoved signal exists, the rail gains no decision,
  and the capture shows its controls and the line saying curation reads it next
  (19a, S224)

#### Scenario: Build now
- **WHEN** the operator uses build now on a capture
- **THEN** a chore unit stands approved on a bolt named from the capture's first
  words, the call is its approval, and the signal's move routes it to the unit
  (19a, 34, S217)

#### Scenario: Make an intent
- **WHEN** the operator uses make an intent on a capture
- **THEN** an intent named from the capture's first words stands open with no
  decision of its own, the signal's move attaches it there, and the intent
  proposes its first elaboration from it (12, 19a, 21)

#### Scenario: Attach to an open intent
- **WHEN** the operator picks an open intent from a capture's attach control
- **THEN** the signal's move attaches it to that intent, and the intent's one
  proposal awaiting approval takes it as material (19a, 21, 116)

#### Scenario: Curation moves the signal first
- **WHEN** curation records a move for a capture's signal
- **THEN** the capture's controls go and the signal keeps curation's move as its
  one move (19a, 107)

### Requirement: Curation is a bounded judgment the operator may make by hand

Curation SHALL decide which signals become intents, and it MAY be a person, an
agent, or both; the flywheel SHALL accept its output whoever produced it (20).
Curation SHALL be charged on a cadence, when unmoved signals exceed a threshold,
or at once on the operator's dictation to run it now, which is one dictation
whether given by a control on the page or a command (110, 12, 193); it SHALL
never open an intent (110). A person writing the same records by hand SHALL be
curation, and the operator's controls on a capture are that hand (110, 19a).

#### Scenario: Run curation now
- **WHEN** the operator runs curation now with fewer unmoved signals than the
  threshold and no cadence due
- **THEN** the curator session is charged at once, and the threshold and cadence
  stand as they were (110, `curation.yaml`)

#### Scenario: Curation proposes, the operator opens
- **WHEN** curation clusters signals into a proposed intent
- **THEN** the intent stands as a proposal on the rail and becomes work only on
  the operator's response (20, 110, 5)

#### Scenario: A proposed intent shows its weight
- **WHEN** a proposed intent is presented
- **THEN** it cites its signals and shows how many, from which sources and over
  what span, counted by event date (109, 118)

#### Scenario: Unmoved signals are visible and never discarded
- **WHEN** signals accumulate without a move
- **THEN** the status view shows their count and age by source and lists every
  one by source and age, grouped by capture, each with its capture's controls
  and the dictation to run curation now beside the trigger that would otherwise
  charge it; while curation runs the listing shows it working and the count
  falls as moves are written; none is discarded (118, 19a, 110)
