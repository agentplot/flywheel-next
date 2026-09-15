## Purpose

The chat the operator already reads: one line per decision with the same number
the page shows, a link back to the object, the numbered reply grammar, and the
notification that reaches a phone.

## ADDED Requirements

### Requirement: The chat carries the same decisions and numbers as the page

A chat rendering of the rail SHALL carry the same decisions and numbers as the
page, one line each, and a link to the page (18). It SHALL keep each kind's form
in one line, so that a decision line is answerable and no other line is (18,
209).

#### Scenario: One line per decision, same numbers
- **WHEN** the rail is delivered to the chat sink and to the page
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
- **WHEN** a decision is delivered to a chat platform that offers answer
  controls
- **THEN** the posted message carries those controls, and a numbered reply to
  the same decision answers it as well (155, 309, 194)

#### Scenario: Buttons carry the answers that take no words
- **WHEN** a decision line is delivered to a sink that declares buttons
- **THEN** it carries one button per answer that takes no words, up to the row
  the platform allows, labelled with the number and the answer, the first
  answer in the affirmative look and `drop` in the destructive one; an answer
  that takes text and any answer past the row stay in the line for the
  numbered reply (S79, 155a, 194)

### Requirement: The sink acknowledges what the operator sent, once, when it is recorded

The operator's proof that a response was recorded SHALL be the sink's
acknowledgement of what they sent (154, S32). A numbered reply SHALL be answered
with a reply on it naming the decision, the answer and the record made. A press
on a control SHALL be acknowledged at once, privately to the one who pressed, and
that acknowledgement SHALL be filled in with the same once the answer is recorded
(S32). A press SHALL reach the sink as the numbered reply it stands for and be
recorded by that grammar, never as a command of its own; who gave it SHALL be the
sink's member where the sink has one, and on a shared channel who wrote the
message (model 5.6, 153, 236, 253a). The acknowledgement SHALL be given once, when
the response is recorded: the same delivery reaching the sink again SHALL write
nothing and SHALL not be acknowledged again, on every channel (model 5.6, 137,
217f).

#### Scenario: A numbered reply is answered on itself
- **WHEN** the operator replies `yes 412` in the channel
- **THEN** the response is recorded, and the sink replies on that message
  naming decision 412, the answer and the record made (154, S32)

#### Scenario: A press is acknowledged before it is recorded
- **WHEN** the operator presses `412 yes`
- **THEN** the press is acknowledged privately at once, the response is
  recorded as `412: yes` would be, and the acknowledgement is filled in with the
  decision, the answer and the record made (S32, 154, 153)

### Requirement: A rendering goes on in as many messages as the platform needs

Where the platform bounds a message, a rendering SHALL go on in as many messages
as it takes, each decision's controls in the message that carries its line, and
the last message SHALL be the delivery the sink's mark records (S32, 14). A
rendering SHALL mention nobody and unfurl no link (S32).

#### Scenario: More decisions than one message holds
- **WHEN** a rendering carries more decisions or more characters than one
  message allows
- **THEN** it is posted as successive messages, no decision's line is split
  from its controls, and the mark records the last message (S32, 14)

### Requirement: The bot's token is read by name and written nowhere else

The bot's token SHALL be read, by the name the manifest gives, from the store the
operator placed it in, and SHALL be written nowhere else: not the manifest, not a
record, not the page, and not an error (217l, 204, 207). A token not yet placed,
or one the platform refuses, SHALL be a decision under attention that names what
to place; the host SHALL run on presenting nothing to that sink, and the sink
SHALL send nothing more until the operator places another (217l, 81, 217k).

#### Scenario: A token not placed
- **WHEN** a host presenting the chat sink starts and the variable the manifest
  names is empty
- **THEN** a decision under attention names the variable, the host runs on and
  serves the page, and the token appears in no manifest, record, page or error
  (217l, 204, 207)

#### Scenario: A token the platform refuses
- **WHEN** the platform refuses the token
- **THEN** the sink sends nothing more, a decision under attention names the
  variable to fix, and nothing is sent again until another token is placed
  (217l, 81)

### Requirement: Replies wait for the presenter and are applied once

What arrives while no host presents a chat sink SHALL wait in the chat and be
applied once by its delivery id when the presenter returns, and acknowledged once,
when it is recorded; a message read again after a restart is the same delivery,
SHALL write nothing and SHALL not be acknowledged a second time (217f, 137, 154).

The platform replays nothing of its own, so the presenter SHALL read what waited:
whenever it opens the wire, at start and again after a loss it could not resume,
it SHALL read the channel after the newest message it has accounted for — at
first the delivery the sink's mark records — oldest first, and hear each message
as if it had just arrived, in the order written with whatever the wire handed
over meanwhile (217f, S32, model 5.5). A message the sink already answered SHALL
not be heard again, a message heard from the wire SHALL not be heard again from
the channel, and a sink that has never delivered SHALL hear nothing written
before its first delivery (217f, 137).

#### Scenario: A reply sent while the laptop slept
- **WHEN** the operator replies `yes 412` while the only presenting host is
  asleep, and the host returns
- **THEN** the reply is read, recorded once and acknowledged once, and nothing
  written before the delivery the sink's mark records is heard (217f, 137, 154)

#### Scenario: A reply sent while the connection was lost
- **WHEN** the presenter's connection is lost past resuming, the operator
  replies meanwhile, and the connection is opened again
- **THEN** the reply is read from after the newest message the presenter had
  accounted for and recorded and acknowledged once (217f, 137, 154)

#### Scenario: A reply read again after a restart
- **WHEN** the host restarts before it delivers again, and reads the channel
  after the same delivery
- **THEN** a message the sink already answered — a numbered reply it recorded,
  or a line it answered with what it accepts — is not heard again, and one that
  reaches the sink all the same writes nothing and gets no second
  acknowledgement (217f, 137, 154)

#### Scenario: A sink that has never delivered
- **WHEN** the presenter opens the wire of a sink whose mark records no delivery
- **THEN** nothing already in the channel is heard, and what waits is read from
  after the sink's first delivery (217f)

### Requirement: The chat accepts two shapes of message and refuses to guess at the rest

The chat sink SHALL accept the numbered reply grammar, which is the answer tool,
and a forwarded message, which is a capture (194, 112, 215). Any other message
SHALL be answered with one reply saying what the sink accepts and SHALL write
nothing, because the machinery never parses free text (194).

#### Scenario: A forwarded message becomes a capture — mirrors S21
- **WHEN** the operator forwards a chat message with one response
- **THEN** one capture exists with a pointer back to the source, one signal
  exists naming that capture, and nothing else happens (111, 112, 215)

#### Scenario: Free text writes nothing
- **WHEN** a message arrives that is neither the reply grammar nor a forward
- **THEN** the sink replies with what it accepts, writes no record, and
  interprets nothing (194)

### Requirement: A decision reaches the phone through the chat sink's notification

A decision raised SHALL reach the operator's phone through the chat sink's own
notification, carrying the answer controls the platform provides and a link to
the object on the page (309). The page SHALL send no push of its own (309).
Every chat rendering, notification and rail line SHALL carry a link at the
host's address with the instance in the path, opening that object in the
dock with its answer controls in reach; a link to a host that is away SHALL say
so rather than failing silently (308, 205a, 150a).

#### Scenario: A new decision reaches the phone
- **WHEN** a decision is numbered and routed to the chat sink
- **THEN** the message posted to the channel carries the number, the platform's
  answer controls and the link to the object, which is what the platform's own
  notification raises; the page posts nothing of its own (309, 308)

#### Scenario: A link to a host that is away
- **WHEN** the operator opens a link to an object held by a host past its stale
  window
- **THEN** the page says the host is away and since when, rather than failing
  silently (308, 150a)

### Requirement: One presenter per sink, one mark per sink

Exactly one presenter SHALL deliver to each sink at a time, held by lease or
pinned by the manifest — "pinned" being 148's own word (148). The lease SHALL be
taken only by a host whose declaration presents the sink and that has opened its
channel (149, model 5.5). Each sink SHALL carry its own delivery mark, so
the tail since the last look is that sink's (14, 148, 236). Sinks SHALL be per
member, and a shared channel SHALL be a sink of its own with one mark (236). A
delivery that fails SHALL be reported under attention with its reason, the mark
SHALL stay where it was, and the next tick SHALL owe the delivery again; nothing
passes silently as not presented (81, 127, model 5.5).

Only the presenter SHALL hear a sink. A message arriving on a chat sink's wire
SHALL be a notify to the host that holds the wire, page served or not, read
before the next pass (217l, model 5.5). What arrives while no host holds the
lease SHALL wait, and be read when the presenter opens the wire; what arrives
while another host holds it SHALL be that host's to hear, so a reply is answered
once however many hosts listen (148, 217f). A message that could not be read,
what waited that could not be read, and whatever stopped the wire, SHALL be
reported under attention (81, model 5.5).

#### Scenario: One delivery, not two
- **WHEN** more than one host could present the chat sink
- **THEN** the holder of that sink's lease delivers and the others do not, so
  the operator sees each delivery once (148)

#### Scenario: A failed delivery is reported and owed
- **WHEN** the presenter's post to the platform fails
- **THEN** the failure and its reason are under attention, the mark has not
  moved, and the next tick delivers again (81, 127)

#### Scenario: A reply is heard by the presenter alone
- **WHEN** two hosts listen on the chat and one holds the sink's lease
- **THEN** a reply arriving wakes the holder's loop and is recorded once, and
  the other host lets it go (217l, 148, 217f)

#### Scenario: Each sink's tail is its own
- **WHEN** the chat sink and the page sink have different marks
- **THEN** each delivery carries the tail since that sink's own mark (14, 236)

### Requirement: The chat is one of the sinks routing by kind reaches

The chat SHALL be one of the sinks the operator may set per kind for the routing
stated in the run record's spec (82).

#### Scenario: A kind routed to the chat
- **WHEN** the operator routes a kind to the chat sink and an event of that kind
  occurs on a host that presents no sink
- **THEN** the chat carries it, and the noticing host shows it nowhere of its
  own (82)
