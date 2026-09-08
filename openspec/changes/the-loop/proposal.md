## Why

The flywheel is specified whole — requirement clauses numbered to 314, a
statechart model of every machine, five phases — and none of it runs against
real work. What exists in this repository is a prototype: the
engine, the register, the tail and every effect the machines ask for are real,
and sessions, git and hosts are faked (README.md). Nothing in it is durable in
133's sense — the store is one file on the laptop, `state/store.json`, which no
loss of that laptop survives — nothing reaches the operator's phone, and no
second host could ever read it.

Phase 1 of the roadmap, **the loop**, is the first of five and the smallest thing
that is a flywheel rather than a demonstration: one organization, one laptop
host, where captures land, the tick evaluates the machines, decisions are raised
and delivered to the page and one chat sink, responses are recorded, and the run
record is readable — with the machines and profiles run from their definitions,
on the git-only profile, and no construction. It is the phase that makes the
operator's response real (1, 137, 153), the state durable (133, 160), the record
readable with no machinery running anywhere (132, 145), and every decision
answerable from a phone (306). Every later phase hangs work off that loop; none
of them can be started until it turns.

The four phases after it, for context, because nothing here may foreclose them:

| phase | name | adds | requirements |
|---|---|---|---|
| 2 | construction | units and bolts landing on willdan repositories — spec, build, review, merge, landing by pull request; the tracker profile (C.1) joining for willdan's board; sessions charged by the machinery on the pane runner; the flywheel instrument | A.17–A.21, A.27 (types) |
| 3 | context | the context map, the books, the claims, the ledger, the OpenSpec artifact views in the dock, packages and the package store, scenario packs | A.14, A.21, A.26–A.28 |
| 4 | dispatch | dispatch as a host with its four jobs, the receiver, the interpreter in the page's browser and in-process, triage placements, several organizations on one host, users and ownership, environments | A.25, A.26, A.29, A.30 |
| 5 | scale | the hosted tiers: identity, the per-tier dispatcher, queues, cache, scheduler, pools, tenancy and encryption, plans and presets, the management console, the MCP endpoint | A.31–A.36 |

A.37 is the control plane, which belongs to flywheel-cloud and not to this
repository (roadmap, Repositories). The binary's own part of it is the
invocation contract as a public document in the open-source repository (297);
the roadmap schedules it in no phase.

**A note on two words.** Part B is the **state store**: durable, shared storage
and the operator's surfaces, reached through a fixed set of operations with
fixed guarantees, and the only thing the data plane touches (125, §3
vocabulary). The **control plane** is the service side of A.37 — Flywheel Cloud
— and is another thing entirely; it appears in this proposal only where a later
phase is named.

## What Changes

### What becomes real in phase 1

- **The state store is a contract, and the git-only profile satisfies it.** The
  operations of 125 — read evidence, write an effect, take/renew/release a
  lease, present and receive, notify, list, serve the status view: seven
  operations, eight methods on the model's `StateStore` trait because present
  and receive are two — become one interface with the five guarantees behind it:
  durable, single writer per object, atomic per write, derivable, the response
  exactly once (133–137). The git-only profile (C.2) binds them to git and
  nothing else: all state is files in git repositories and the git host is the
  only central service (160). There is no tracker; issues, milestones and boards
  may exist for people, and the machinery neither reads nor writes them (C.2).
- **A change of state is a commit.** A commit that reaches the shared line is
  the fact; a commit that has not is a local intention (161). Every write
  carries its effect's identity, its reason and the evidence it was based on, so
  history is the audit record and nothing else is kept for that purpose (127,
  167, 79). A repeat of an effect already written changes nothing, is not an
  error, and is not reported as a second write (127).
- **The push is the compare-and-swap.** Two writers of one object cannot both
  succeed, because the git host rejects an update whose base is stale; the loser
  learns it lost and reads again before deciding anything (134, 162, I15). A
  write is wholly applied or not applied and no reader sees half of one (135). A
  lease is taken by a commit that lands, renewed while the host works, and
  expired by a stated rule (128, 163). The profile keeps leases and heartbeats
  off the shared line — one orphan commit on `lease/<id>` and `host/<id>`,
  replaced with `--force-with-lease`, the old commits unreachable — so renewals
  every minute for months add nothing to `main` (`profiles/git-only.yaml`
  `layout.leases`, `layout.hosts`; model.md §4.2).
- **The definitions rule.** The shipped machines and profiles are inside the
  binary as core machines, never edited by an organization, each carrying its
  version and named by the release's set version (223, 224, 83; model.md §13);
  an organization's own types and packages are read from the blueprints
  repository at the shared line, so a type composed of existing atoms is added
  with no code change and no host is rebuilt for one (57, 85, 228). The engine
  evaluates predicates over evidence, chooses transitions, runs effects idempotently and derives the plan's
  decisions, and holds no name of any object it drives (86, 87). Every evidence
  and effect name is abstract in the definition and bound by the profile
  (138–140, I13).
- **The tick is the scheduler.** One long-lived host process, started by the
  platform's own launcher, fetches and integrates the shared line before every
  tick so no host decides on a read older than the bound and no person runs the
  sync by hand (165), then lists, reads, evaluates, performs the effects whose
  proof is absent, and commits what it did. Every timed behaviour — adapters
  included — is a guard on that tick and nothing else keeps time; a run missed
  while the host was down is caught up on the next tick, and the idempotent key
  makes the catch-up write nothing twice (231). Reading twice with nothing
  changed writes nothing (78, 126); a restart changes no state and no plan
  (7, 75, I7, I14).
- **Notify, and what a laptop defaults to.** Hosts learn of new state without
  reading the whole history each time, within a stated latency bound (130, 166).
  On a laptop the default is the bounded poll — one `git ls-remote` round trip
  every 30 seconds, no history read; the git host's call when a branch moves
  requires an address reachable from outside, so it is the operator's opt-in,
  never the default, because publishing an address beyond the private network is
  the operator's choice (46, 191, section 9).
- **Captures land.** Adapters append captures unattended — one keyed capture per
  source event with its provenance and a pointer to raw material that stays
  outside every repository (111, 215) — and capture costs one gesture from
  wherever the operator is (112). Phase 1 ships three, all of them enumerator-only
  arithmetic that starts no session (115): the page's capture box, which writes
  a capture with one signal of kind ask so curation sees it, with marking it an
  intent a control and never a word parsed out of the text (19, 194); the chat
  forward, one response producing one capture and one signal with a link back
  (112, 215, S21); and the meeting transcript, one keyed capture per file, so a
  transcript imported twice yields one capture (111, 215, S22). Their judgment
  half — turning a capture into signals — is curation's and never runs
  unattended (115). The folder drop and the log or monitor webhook follow with
  their sources. Signals are immutable, carry
  their kind, asserter, subject, assertion and verbatim excerpt (113), and each
  takes exactly one standing move with a stated consequence (107, 116). The
  signal and move formats are versioned and stable, so captures made before the
  flywheel existed read without conversion (114). Signals, moves and captures
  are files in the blueprints repository under the machinery's own prefix (203).
- **Intents and their elaborations.** Curation, not the elaboration machinery,
  decides which signals become intents, and the flywheel accepts curation's
  output whoever produced it (20). An intent carries at most one elaboration
  awaiting approval and new material joins that proposal (21); its close is
  proposed when all its elaborations are done and only the operator closes it
  (22). An elaboration is one session, one conversation, one place, never split
  by the machinery (24); the three types differ in how they end — self-closing,
  standing, with-operator — and a standing session is never ended by the
  machinery because it went idle, because its items closed, or because a restart
  forgot it (25, 26, I6); the type is chosen when the elaboration is proposed and
  correctable by the operator's response (27).
- **Decisions are raised, numbered, and delivered.** The plan is derivable from
  current state and nothing a process remembers (7), always current (8), and a
  decision appears exactly when the choice becomes the operator's and disappears
  when it is made or can no longer be made (9, I3). Every decision carries a
  short number unique in the organization, given once and never reused, the same
  on the page and in chat, with no rendering stored (15). Decisions group so
  that "yes to all" means something and any one can be answered alone (11).
  Delivery is to two sinks — the page and one chat sink — each with its own
  delivery mark as recorded state, so the tail of what reached done, landed,
  closed or dropped since that sink's last look is derivable like the decisions
  (14, 18, 148, 236).
- **The phone is a first-class surface, not a port of one.** Desktop and phone
  ship together from one bundle: every decision is answerable on either, and
  every control and every form the page carries is available on both — nothing
  the page offers is desktop-only and nothing the phone answers is missing on
  the desktop (306, 307, 2, 155). The phone is that same served
  bundle under 760px and never a second application: two tabs, Decisions and
  Board, with the dock full screen and a back control — the desktop's metaphor at
  a smaller size (307). Every chat rendering, every notification and every plan
  line carries a link to the object on the page at the host's address with the
  organization in the path, opening that object in the dock with its answer
  controls in reach, and a link to a host that is away says so rather than
  failing silently (308, 205a, 150a). A decision raised reaches the phone through
  the chat sink's own notification with the platform's controls and that link;
  the page sends no push of its own, and the short reply grammar always works
  beside the controls (309). The status view renders from one request, the page
  holds no client state a reload loses, so a reload after an answer shows the
  answer recorded with who gave it and when, and the bundle fetches nothing from
  anywhere else (310, 153, 154). Every answer is one tap or one short reply;
  nothing is reachable only by hover or keyboard, and a long-form answer is given
  with the platform's own keyboard (311).
- **Responses are recorded, and the response becomes a commit.** One place,
  applied exactly once (1, 137, I2); a short chat reply or a choice on the page
  is sufficient for any decision (2, 152); the profile names the one writer that
  turns either into the commit, and how the operator can tell the commit landed
  (154, 164). The operator's own commit against the state where it is kept is
  the response too (3, S19). A response that cannot be applied is reported,
  never dropped (6, 129). Nothing the operator has not approved exists as work
  (5, I1). The operator may invoke by dictation any transition that undoes or
  defers work and none that asserts work was done, and a session ended by hand
  is read as a session gone (4); their own dictation skips the plan (12).
- **Every operator operation is a tool.** Capture, mark as intent, answer a
  decision, drop, later, hold, rename, finish a session, and every other
  transition clause 4 grants is exposed by the state store as a tool with a
  schema naming its arguments by object id; the page's controls, the chat and
  the machinery all call the same tools and no caller has an operation the
  others lack (193). The machinery never parses free text (194). Phase 1 serves
  that one catalogue over HTTP for the page; it is one object served in more
  shapes as later phases bring clients that need them.
- **Sessions, as far as phase 1 goes.** A session is given one job, one place
  and a bounded goal, with a fixed set of exits (65), and the machinery decides
  what an exit means (66). It reports that exit through a command the machinery
  provides — `flywheel exit | offer | note | refuse` — which writes to the state
  store, and nothing it leaves on the place's disk is state (67). That command
  is real in phase 1, because the stand-in of 93 plays every scripted exit
  through the very same path (`profiles/sessions-stand-in.yaml`). The operator
  may open a session of their own at any time, on no thread, ending by dictation
  (69, X01). The machinery never interrupts a session that is working (71, I5);
  a slow start is slow, not failed, is retried, and is judged by evidence that
  the session exists (72); every action on a session is safe to repeat (73); and
  a session that is not the operator's to keep is retired when the work it
  serves is retired (74).
- **Findings.** A session may offer a finding at any time; a finding about its
  own intent is a proposal on the plan for that thread, and a finding about
  anything else is a signal, neither work until the operator says so and neither
  interrupting the session that offered it (58). The machinery keeps one record
  in state pointing at the document, and the record never holds the text (62).
- **The run record is readable.** Every write is recorded with its reason and
  the evidence it was based on (79, 167); what was expected of a session and
  what was delivered are both recorded, difference first (80); problems with the
  machinery are reported through this record and never filed as work (81); and
  notifications are routed by kind to sinks the operator sets per kind, so
  nothing the machinery notices is visible only on the host that noticed it
  (82). The status view is a projection of the same state the engine reads,
  derived from list and read alone, central, reachable from a phone, and
  readable with no machinery running — as a file of the shared line, saying as of
  when (141–146, 132, S20). Discussion about an object is part of its state and
  the view shows it (144). Each kind of object has one form and no two share one
  (209), and an elaboration is a surface of its own reached from its intent
  (210).
- **The organization and its host are objects.** `flywheel init` drives the
  organization machine, whose bootstrap states 204 names as absent, blueprints
  ready, state ready, connected and hosted, and whose loaded definition adds
  three more: `awaiting-app`, the attention decision that stands while the
  installation is unseen and is never done by an agent (204, 82), and
  `removing` and `removed`, which the dictation `remove <organization>` enters
  (4, 221). The machine ticks unchanged, so that dictation is in the tool
  catalogue from day one (193); phase 1 honours it — ending sessions, removing
  places, archiving the state and leaving the git repositories on disk, with
  the numbers never reused (221, 15) — because refusing a transition the loaded
  machine offers would be the machinery deciding what the operator may undo
  (4). It creates or adopts the
  blueprints repository from its template, creates the state repository with the
  profile's layout (204, C.2), records that the GitHub App must be installed as a
  secret the operator places, and registers the first host, which is what carries
  the organization to `hosted`: bootstrapped, one host registered — not a hosted
  tier. Every step is an effect with a proof, so running it again changes nothing
  and the reconciler advances a half-finished bootstrap (204). A host joins by
  one command and never by hand, cloning the state, the blueprints and every
  tracked built repository as bare repositories under one root the manifest
  names, keeping one checkout of each shared line for the machinery's own merges,
  and refusing to start on a hand-made layout, saying what differs (205); it has
  one address with the organization in the path (205a). One GitHub App is the
  connection; no host or session uses a personal token (207, 207a). The template
  set is versioned and stamped at initialization (208).
- **Hosts, leases and ownership.** A host declares what it takes and takes
  leases only within its declaration; an object no declaration covers is a
  decision under attention, not a silent wait (149). Every object is owned by at
  most one host through a lease and the owner is visible (150, I11). A laptop is
  intermittent by default: past its stale window it shows as away with
  since-when, raises no attention line, keeps its leases, pauses its sessions'
  stall clocks, and is alive again on its next heartbeat with nothing to answer
  (150a). A host that cannot reach the git host keeps working what it already
  owns, commits locally, takes no lease, starts nothing new, delivers to no
  sink, and reconciles on reconnect (151, 165). One laptop is what phase 1 runs,
  and two host processes are what phase 1 tests (S13, S17, S18).
- **Where files live is enforced.** Three repositories, three owners: the state
  repository is the machinery's alone, laid out as the profile says; the
  machinery writes in the blueprints repository only under `flywheel/` and the
  change directory an intent opens; tracked flywheel-facing files sit under
  `flywheel/` and untracked per-place files under `.flywheel/`; and the
  machinery never writes outside its prefix except as the effect of a response
  (203). No state exists outside git: a host's memory and disk hold only what
  git already holds or what is about to be committed, and evidence a host
  observes about the world each tick is re-observed after a restart (I14, 75).
- **Endpoints are the host's binding.** The machinery derives a port from the
  place and asks the host's router for the URL it records; phase 1 is the local
  router on the operator's machine (191, 45, 46). Nothing is published beyond
  the operator's private network unless the operator says so (46).
- **Instructions are data.** No instruction text exists in the engine and no
  engine behaviour depends on an instruction's wording (119); a session is given
  the versions in force when it starts and its inputs are enumerable and closed
  (88, 89); a test renders the exact prompt a scenario would produce, without
  starting a session (90, 124); the instructions live in the shipped set and
  reach every host with it (91, 208), and each is versioned so a session started
  before a change and one started after can be told apart (123).
- **Coexistence.** The new flywheel runs beside the current one against the same
  organization with a disjoint, explicit scope of objects (96).

### What stays faked, and what waits

Every section of the roadmap's phase-1 row, and where it stands:

| section | phase 1 |
|---|---|
| A.1 the operator's response (1–6) | real |
| A.2 the plan (7–19) | real |
| A.3 intents and curation (20–22) | real; the design book of 23 is phase 3 |
| A.4 elaborations and their types (24–27) | real |
| A.5 planning and construction (28–57) | the machines tick, unit types included, with the effects of 42 as store facts; the host binding of those effects and the pane runner are phase 2 |
| A.6 findings and chores (58, 62) | real; chores as units of the chore type (59–61, 63, 64) are phase 2 |
| A.7 sessions (65–67, 69, 71–74) | real, with the runner a stand-in; 68 (never opening a pane) and 70's living session need the pane runner, and 197 (the tool server refusing a call whose identity is not the pane's) needs panes — phase 2 |
| A.8 state and evidence (75–78) | real |
| A.9 observability (79–82) | real |
| A.10 the engine and the model (83–87) | real |
| A.11 instructions and skills as data (88–91) | real; a change to one is a chore, so the chore half of 91 is phase 2 |
| A.12 scenarios and testing as data (92–94) | real; 95's dictation-into-data half needs the interpreter — phase 4 |
| A.13 coexistence (96) | real |
| A.14 claims, as-built, the ledger (97–105, 195) | phase 3 |
| A.15 signals and curation (106–118, 215) | real, with the phase-1 adapters named above |
| A.16 default instructions (119, 123, 124) | real; 120–122 write the chapter, the claim and the context map and derive the review view from them, and 198–202 are the map — phase 3 |
| A.22 endpoints and routing (191) | real, local router |
| A.23 where files live (203) | real |
| A.24 bootstrapping (204, 205, 205a, 207, 207a, 208) | real; 206 creates a repository with its map nodes and homes and runs the first planning's baseline, so it is phase 3, and phase 1 adopts a repository by its git details alone (199) |
| A.38 the phone (306–311, 314) | real; 312 is the management console (phase 5) and 313 is the mobile tool-server client, which needs 293 (phase 5) |
| B the state store (125–155, 209, 210, 236, 236a) | real; 213's OpenSpec artifact views need the book and the map (phase 3) and 214's instrument is phase 2 |
| C.2 the git-only profile (160–167) | real |
| C.1 the tracker profile (156–159) | phase 2, for willdan's board |

What that leaves faked:

- **Sessions.** The exits are real as a contract and the exit command is real
  (65, 66, 67), but no agent runs them. The runner that charges a session on a
  multiplexer pane is A.17 and lands in phase 2 (171). Phase 1 plays every
  session from a script through the same command path a real runner will replace
  (93). Where phase 1 needs curation without a runner it has one: a person
  writing the same records by hand is curation (110), and the enumerator half of
  every adapter runs unattended while turning a capture into signals stays a
  judgment that never runs unattended (115).
- **Git for lines, places, merges and landing.** Real git in phase 1 is the
  state repository as the store (160–167), the blueprints repository, and the
  machinery's own writes under its prefix (203, 204, 205). Everything in A.5
  that changes what a branch or a working place is — creating a line, preparing
  or removing a place, merging into the bolt, landing (42, 49–55) — stays a fact
  in the store until phase 2.
- **Construction reaching a repository.** One rule covers every machine phase 1
  does not fully bind: every shipped machine ticks, unit types among them (37,
  223), and the effects of 42 — creating a line, preparing or removing a place,
  merging into the bolt, landing — are bound to the stand-in world as facts in
  the store, exactly as the prototype binds them (README.md, 93). So a bolt, a
  unit, a work item and a stage do move through their states in phase 1, and
  nothing they do reaches a repository. What phase 2 adds is the host binding of
  those effects, to `wt` and git, and the pane runner behind the sessions. Under
  that rule planning (28), the unit proposal document (36, 17) and pull-request
  landing (53, A.18) still wait on the runner, and the flywheel instrument (214)
  on the acceptance files a landing writes.
- **Two adapters of 215 never run on this profile.** The pull-request
  conversation and the issue tracker read the git host's issues and reviews, and
  C.2 forbids the machinery reading them; they belong to the tracker profile in
  phase 2. The capture endpoint of 215 is for callers that cannot reach any
  host's binary and is one of dispatch's four jobs, so it is phase 4 (216).
- **The tracker profile.** C.1 is a second implementation of the same contract
  and joins in phase 2 (156–159). Phase 1 admits it by construction and does not
  build it: the machines do not change between profiles (139), and a profile is
  admitted when its binding is complete and the conformance suite passes
  unchanged (140, 168).
- **Claims, the ledger and the context map.** A.14 and the map (195, 198–202)
  are phase 3. Phase 1 keeps only what stops them being foreclosed: a repository
  record holds git details alone and never a hand-declared kind, capability or
  scope (199), and no claim is planned against (98).

## The acceptance

A phase ends when its scenarios pass in conformance (roadmap, phase gates), so
phase 1 names them. The suite is the model's `conformance/`, run by `flywheel
scenario run` with the machine files byte-identical and their hash in the run
record (168, section 12). Every scenario below that carries an operator's
response also runs at a 390px viewport, and the mockups render at 390px (314).

**`contract/` — all thirteen, both paths.** `read`, `write-effect`, `lease`,
`present-receive`, `notify`, `list`, `status` (the operations of 125–132);
`durable`, `single-writer`, `atomic`, `derivable`, `response-once` (the
guarantees of 133–137); and `binding` (139, 140, 168). They run over the toy
`lamp` machine, which shares no atom with the flywheel, so the profile is tested
before the domain loads — first against the stand-in store, then against a local
bare state repository with no network. This is the set that admits the git-only
profile, and it is the heart of the gate.

**`scenarios/` — twenty-one in phase 1.**

| scenario | path | what it proves here |
|---|---|---|
| S01 approve an elaboration from the phone | both | one response starts work, nothing re-asks, at 390px (1, 6, 13, 306, 314) |
| S02 standing prototype idle | both | finish-or-keep, never ended by the machinery (25, 26, I6) |
| S04 a finding on its own intent, dropped | both | nothing was created (58, 5, I1) |
| S05 restart mid-day | both | the plan is identical, no object moved (7, 75, I7) |
| S06 a slow start | both | slow is not failed; it starts once (72, 73) |
| S07 the intent's close | both | the close decision and its one response; the line's archive is a store fact until phase 2 (22, 13) |
| S08 twenty signals curated | both | every signal has exactly one move; two decisions, not twenty (107, 109, 116) |
| S13 two hosts, one loses power | both, two host processes | stale is shown, the other does not touch it, takeover by rule, never twice (150, 150a, I11) |
| S16 a scenario as data | stand-in | it runs and renders as a trace; the dictation-into-data half waits for the interpreter (94, 95, phase 4) |
| S17 two hosts race for one object | git-only, two host processes | exactly one takes it; the loser reads again (134, 162, I15) |
| S18 a host's network drops for an hour | git-only, two host processes | it finishes what it owned, commits locally, reconciles (151, 165) |
| S19 the operator edits a state file by hand | git-only | the next pass on every host treats it as the response (3, 164) |
| S20 the status page six hours after the last host stopped | git-only | the state as of the last landed commit, saying so (132, 145) |
| S21 a forwarded chat message | both | one capture, one signal, a link back, nothing else (112, 215) |
| S22 the same transcript twice | both | one capture, its signals read once (111) |
| S23 a proposed intent dropped | both | it is not re-proposed; each signal's move names the drop (117) |
| S24 a dropped signal revived | both | the move is replaced and the next run clusters it (107) |
| S29 three items, a bound of two | both | the third waits, starts when a slot frees, nothing twice across a restart (31, 32) |
| X01 the operator's own session | both | no decision is ever raised about it; it ends by dictation (69) |
| X05 a unit of a type no host declares | both | an attention decision, not a silent wait; a host takes it when the manifest covers it (149, 150) |
| X08 an in-flight unit dropped by dictation | both | its sessions retired and places released; a dictation asserting work done is refused and reported; a pane killed by hand is a session gone (4, 12, 66, 74) |

The two host processes of S13, S17 and S18 cost nothing: the stand-in's `hosts`
binding already starts a second `flywheel host` as its own process on one
laptop, with its own root and port range, and can lose it, disconnect it or
return it (`profiles/sessions-stand-in.yaml`).

**Deferred, with the reason.**

| scenarios | waits for |
|---|---|
| S03, S28, S30, S31 | a session that reads a document, answers a question or reports work done — the pane runner, phase 2 |
| S12, S14, S15, S26, S32, S33, S34, X03, X06, X09 | assertions about real merges, rebases, conflicts, landings, panes or operator-added types — phase 2 |
| X07 | the bell on a named surface needs the multiplexer — phase 2 |
| T01 | the tracker profile — phase 2 |
| S09, S10, S11, S25, S27, X02 | claims, the ledger, planning and the map — phase 3 |
| X04 | the dispatcher as a presenter — phase 4 |

Of the four rules those deferred scenarios also carry, two are proved in phase 1
by scenarios above: the attention decision for an object no declaration covers
by X05 (149), and the refusal of a dictation asserting work done by X08 (4).
The other two are built in phase 1 and proved later, because no phase-1
scenario's `satisfies:` names them: one presenter per sink is the page and chat
sinks' leases (148), which X04 proves when a dispatcher exists to contend for
one, and routing by kind reaches the page and the chat in phase 1 (82), which
X07 proves when the bell's surface exists.

## Capabilities

### New Capabilities

- `engine/definitions-and-tick`: the shipped machines and profiles inside the
  binary, the organization's types read from the blueprints, guards, regions,
  submachines, effects with proofs, idempotent repeat, the tick as the only
  clock, and the engine/domain line (57, 83–87, 138–140, 75–78, 228, 231).
- `engine/plan`: decision derivation from active states, the register and its
  numbers, grouping and folding, the decision kinds, the tail and the sinks'
  marks (7–19).
- `state-store/contract`: the operations of 125 and the guarantees of 133–137 as
  one profile-neutral interface — the `StateStore` trait — that every caller
  goes through (125–137).
- `state-store/git-only-profile`: the complete binding of that contract to the
  state repository — the layout, the commit per effect, the push as
  compare-and-swap, leases and heartbeats off the shared line, the response
  commit, notify with the poll as the laptop's default, and the committed status
  page (160–167, 139, 140).
- `tools/tool-server`: the tool catalogue — one tool per operator operation,
  arguments by object id — served over HTTP as the single write path for every
  caller (193, 194, 4, 12).
- `surfaces/plan-page`: the page sink as one bundle that is the phone under
  760px — the plan, the capture box, the status view, the object forms, the
  elaboration surface, one-request status, one-tap answers, no client state a
  reload loses — served on the operator's private network (19, 141–146, 155,
  209, 210, 306, 307, 310, 311).
- `surfaces/chat-sink`: one chat sink carrying the same decisions and numbers
  one line each, its own notification with the platform's controls and a link to
  the object on the page, the numbered reply grammar beside them, and
  notification routing by kind (18, 82, 152–155, 236, 308, 309).
- `signals/capture-and-curation`: captures, signals, moves and their
  consequences; the page capture box, the chat forward and the meeting
  transcript as the phase-1 adapters; curation's records and its cadence
  (106–118, 215).
- `organization/bootstrap`: the organization machine, `flywheel init`, the
  blueprints and state repositories, the GitHub App, host join, and the
  versioned template set (204, 205, 205a, 207, 207a, 208, 203).
- `hosts/ownership`: the host object, its declaration, its heartbeat, leases and
  takeover, the intermittent laptop's away window, and the disconnected host
  (147–151, 150a, 165).
- `observability/run-record`: every write with its reason and evidence, expected
  against delivered, machinery problems reported and never filed as work
  (79–82, 167).
- `scenarios/conformance`: scenarios as data beside the definitions, the
  stand-in store and the scripted session binding through the real exit command,
  the second host as a process, the rendered trace, the run against a local bare
  state repository, and the 390px pass (67, 84, 92–94, 168, 314).

### Modified Capabilities

None. `openspec/specs/` is empty; every capability above is the first spec for
its area.

## What must not be foreclosed

Phase 1 is the seed for four more phases, so these hold from day one:

- **The definitions stay the model's mirror.** `definitions/` is copied from
  `models/statechart/`, never hand-edited; the model changes first, `check.py`
  reports every cited clause, and the copy follows (83, AGENTS.md). A machine
  definition names no store, service, path or field (I13, 138).
- **A second profile is admitted without touching a machine.** The contract is
  profile-neutral and the machines are byte-identical between bindings (139), so
  phase 2's tracker profile is a second implementation of the same interface,
  admitted when its binding is complete and the conformance suite passes
  unchanged (140, 168–170). Parts A and B do not change to admit it (Part C).
- **The tool server is the one write path.** Because the page's controls, the
  chat, the machinery and — later — the dispatch agent and a member's own client
  all call the same catalogue (193), phase 2's sessions, phase 4's interpreter
  and phase 5's remote MCP endpoint are new clients of an existing server, not a
  second write path (194, 291, 293). Phase 1 builds the catalogue as one object
  and serves it over HTTP; the stdio and in-process shapes arrive in phase 2
  beside the runner that needs them.
- **Hosts and sinks are objects.** A host is a declaration (149) and a sink is
  an object with a mark (14, 148), so phase 4's dispatch — a host whose
  declaration takes no object kind, no repository and no unit type and presents
  the chat sink (217) — and phase 5's per-tier dispatcher are declarations
  against machines that already exist. Sinks are per member from the start, even
  with one member (236, 236a).
- **A tick is a bounded invocation.** Nothing held in a process's memory decides
  behaviour after a restart (75, I14), every decision is derived from what read
  and list return (136, 217a), and every timed behaviour is a guard on the tick
  (231). That is what lets phase 5's control plane invoke this binary in tick
  mode — organization, tier, queue messages, scheduler entry, scratch directory
  with a budget, returning the plan delivered, the shared lines pushed by
  compare-and-swap, the messages acknowledged, one next due time and the run
  record — without the binary changing (297, 298).
- **One bundle, and the page's data is a projection.** One bundle is built and
  one is served, and its version is the binary's (307); the status view is
  written from state and never read as truth (142, 76), so phase 5's page
  projection object — the status view and the rail as data with each member's
  sink and mark, rendered from one request — is the same write to a different
  place (291, 298, 310).
- **Same bytes everywhere.** No behaviour is gated at build time and no tier is
  compiled in; a hosted host differs from a laptop only in what its manifest
  binds — a tier, an identity kind and a router (299). Nothing that assumes a
  multi-tenant service belongs in this repository (296, AGENTS.md).
- **The contract is the only door.** The data plane touches nothing but the
  operations of 125, so phase 5's tenancy and encryption (A.33) apply to one
  write path, and a control plane built to the invocation contract by anyone
  else drives this binary through 297's two modes and 298's five shapes and
  nothing more (299).

## Impact

- **Crates.** The four crates named in AGENTS.md grow toward the model's crate
  boundary (model.md §13) by addition, not widening: `flywheel-atoms` (the
  evidence and effect registries generated from `atoms.yaml`, the `StateStore`,
  `World` and `Sessions` traits), `flywheel-domain` (the shipped machines, the
  type catalogue loader, the recutils reader and writer, the work order
  renderer), `flywheel-world-host` (git and the local router),
  `flywheel-store-git` (the `StateStore` over the state repository), and
  `flywheel-surface` extended with the chat sink and the tool server.
  `flywheel-store-tracker` follows in phase 2. `flywheel-engine` keeps no string
  from `atoms.yaml`, checked by grep (86, I13).
- **Definitions.** No hand edits. Any clause phase 1 needs that the requirements
  do not have is proposed to `blueprints` first (AGENTS.md).
- **Naming.** This change is named for the phase it delivers, as AGENTS.md and
  the roadmap's flywheel-next row name it; phases 2 to 5 will be
  `construction`, `context`, `dispatch` and `scale`.
- **External systems.** One git host, depended on for exactly what section 9
  grants it — one update to a branch at a time, rejection of a stale base, and a
  call to a URL when a branch moves — and for nothing else (160, 162, 166). One
  GitHub App as the connection, its key placed by the operator (207, 207a). One
  chat channel that reaches the operator's phone, and a served page on the
  operator's private network (155, 191). Secrets are read from where the
  operator put them and never appear in configuration or code (204, 207).
- **The prototype.** `scenarios/plan-mockup.yaml` and the stand-in store stay as
  the test path (93); the stand-in stops being the only path, and
  `state/store.json` stops being where anything durable lives (133, 160).
- **Not touched.** The existing flywheel keeps running willdan unmodified until
  phase 2 (96, roadmap).
- **Gate.** `cargo test`, plus the conformance suite above: all thirteen
  `contract/` files and the twenty-one scenarios, on the stand-in and against a
  local bare state repository with no network, with the responses among them
  also run at 390px (92, 168, 314, AGENTS.md).
