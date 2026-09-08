## Why

The flywheel is specified whole — 305 requirement clauses, a statechart model of
every machine, five phases — and none of it runs against real work. What exists
in this repository is a prototype: the engine, the register, the tail and every
effect the machines ask for are real, and sessions, git and hosts are faked
(README.md). Nothing in it is durable, nothing reaches the operator's phone, and
nothing survives the loss of the laptop.

Phase 1 of the roadmap, **the loop**, is the first of five and the smallest thing
that is a flywheel rather than a demonstration: one organization, one laptop
host, where captures land, the tick evaluates the machines, decisions are raised
and delivered to the page and one chat sink, responses are recorded, and the run
record is readable — with the machines and profiles run from their definitions,
on the git-only profile, and no construction. It is the phase that makes the
operator's response real (1, 137, 153), the state durable (133, 160), and the
record readable with no machinery running anywhere (132, 145). Every later phase
hangs work off that loop; none of them can be started until it turns.

The four phases after it, for context, because nothing here may foreclose them:

| phase | name | adds | requirements |
|---|---|---|---|
| 2 | construction | units and bolts landing on willdan repositories — spec, build, review, merge, landing by pull request; the tracker profile (C.1) joining for willdan's board; sessions charged by the machinery on the pane runner; the flywheel instrument | A.17–A.21, A.27 (types) |
| 3 | context | the context map, the books, the claims, the ledger, the OpenSpec artifact views in the dock, packages and the store, scenario packs | A.14, A.21, A.26–A.28 |
| 4 | dispatch | dispatch as a host with its four jobs, the receiver, the interpreter in the page's browser and in-process, triage placements, several organizations on one host, users and ownership, environments | A.25, A.26, A.29, A.30 |
| 5 | scale | the hosted tiers: identity, the per-tier dispatcher, queues, cache, scheduler, pools, tenancy and encryption, plans and presets, the management console, the MCP endpoint, and the control plane the binary is invoked by | A.31–A.37 |

## What Changes

### What becomes real in phase 1

- **The control plane is a contract, and the git-only profile satisfies it.**
  The eight operations — read evidence, write an effect, take/renew/release a
  lease, present and receive, notify, list, serve the status view (125–132) —
  become a trait with the five guarantees behind it: durable, single writer per
  object, atomic per write, derivable, the response exactly once (133–137). The
  git-only profile (C.2) binds them to git and nothing else: all state is files
  in git repositories and the git host is the only central service (160). There
  is no tracker; issues, milestones and boards may exist for people, and the
  machinery neither reads nor writes them (C.2).
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
  expired by a stated rule (128, 163). Leases and heartbeats stay off the shared
  line so months of renewals add nothing to its history (163, 167).
- **Machines and profiles run from their definitions.** `definitions/` is
  loaded, not compiled in: the engine evaluates predicates over evidence,
  chooses transitions, runs effects idempotently and derives the plan's
  decisions, and holds no name of any object it drives (83–87). Every evidence
  and effect name is abstract in the definition and bound by the profile
  (138–140, I13). Adding an elaboration type or a decision kind is a definition
  change and no code change (85).
- **The tick is the scheduler.** One long-lived host process, started by the
  platform's own launcher, fetches and integrates the shared line before every
  tick so no host decides on a read older than the bound and no person runs the
  sync by hand (165), then lists, reads, evaluates, performs the effects whose
  proof is absent, and commits what it did. Every timed behaviour — adapters
  included — is a guard on that tick and nothing else keeps time; a run missed
  while the host was down is caught up on the next tick, and the idempotent key
  makes the catch-up write nothing twice (231). Reading twice with nothing
  changed writes nothing (78, 126); a restart changes no state and no plan
  (7, 75, I7, I14). Hosts learn of new state without reading the whole history
  each time, by a call from the git host or a bounded poll, within a stated
  latency bound (130, 166).
- **Captures land.** Adapters append captures unattended — one keyed capture per
  source event with its provenance and a pointer to raw material that stays
  outside every repository (111, 215) — and capture costs one gesture from
  wherever the operator is (112). The page's capture box writes a capture with
  one signal of kind ask so curation sees it, and marking a capture as an intent
  is a control, never a word parsed out of the text (19, 194). Signals are
  immutable, carry their kind, asserter, subject, assertion and verbatim excerpt
  (113), and each takes exactly one standing move with a stated consequence
  (107, 116). The signal and move formats are versioned and stable, so captures
  made before the flywheel existed read without conversion (114). Signals, moves
  and the captures' records are files in the blueprints repository under the
  machinery's own prefix (203).
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
- **Responses are recorded, and the response becomes a commit.** One place,
  applied exactly once (1, 137, I2); a short chat reply or a choice on the page
  is sufficient for any decision (2, 152); the profile names the one writer that
  turns either into the commit, and how the operator can tell the commit landed
  (154, 164). The operator's own commit against the state where it is kept is
  the response too (3). A response that cannot be applied is reported, never
  dropped (6, 129). Nothing the operator has not approved exists as work (5,
  I1). The operator may invoke by dictation any transition that undoes or defers
  work and none that asserts work was done (4), and their own dictation skips
  the plan (12).
- **Every operator operation is a tool.** Capture, mark as intent, answer a
  decision, drop, later, hold, rename, start or stop a service, finish a
  session, and every other transition clause 4 grants is exposed by the control
  plane as a tool with a schema naming its arguments by object id; the page's
  controls, the chat and the machinery all call the same tools and no caller has
  an operation the others lack (193). The machinery never parses free text (194).
- **The run record is readable.** Every write is recorded with its reason and
  the evidence it was based on (79, 167); what was expected of a session and
  what was delivered are both recorded, difference first (80); problems with the
  machinery are reported through this record and never filed as work (81); and
  notifications are routed by kind to sinks the operator sets per kind (82). The
  status view is a projection of the same state the engine reads, derived from
  list and read alone, central, reachable from a phone, and readable with no
  machinery running — as a file of the shared line, saying as of when (141–146,
  132). Each kind of object has one form and no two share one (209), and an
  elaboration is a surface of its own reached from its intent (210).
- **The organization and its host are objects.** `flywheel init` drives the
  organization machine — absent, blueprints ready, state ready, connected —
  creating or adopting the blueprints repository from its template, creating the
  state repository with the profile's layout (204, C.2), recording that the
  GitHub App must be installed as a secret the operator places, and registering
  the first host; every step is an effect with a proof, so running it again
  changes nothing and the reconciler advances a half-finished bootstrap (204). A
  host joins by one command and never by hand, cloning the state, the blueprints
  and every tracked built repository as bare repositories under one root the
  manifest names, keeping one checkout of each shared line for the machinery's
  own merges, and refusing to start on a hand-made layout, saying what differs
  (205); it has one address with the organization in the path (205a). One GitHub
  App is the connection; no host or session uses a personal token (207, 207a).
  The template set is versioned and stamped at initialization (208).
- **Hosts, leases and ownership are specified for one host.** A host declares
  what it takes and takes leases only within its declaration; an object no
  declaration covers is a decision under attention, not a silent wait (149).
  Every object is owned by at most one host through a lease and the owner is
  visible (150, I11). A laptop is intermittent by default: past its stale window
  it shows as away with since-when, raises no attention line, keeps its leases,
  pauses its sessions' stall clocks, and is alive again on its next heartbeat
  with nothing to answer (150a). A host that cannot reach the git host keeps
  working what it already owns, commits locally, takes no lease, starts nothing
  new, and reconciles on reconnect (151, 165).
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
- **Scenarios are the acceptance.** Every machine is testable on its own against
  a stand-in control plane with no live service (92, 84); the whole machinery
  runs with only the session binding replaced by a stand-in playing scripted
  exits, so seeding a scenario exercises the stores, the engine, the git
  effects, the plan and the page with no agent running (93); a scenario is data
  living beside the definitions it checks (94) and renders afterwards as a trace
  a person reads (95). The suite runs against the stand-in and then against the
  git-only profile in a local bare repository with no network, with the machine
  files byte-identical. Each `scenarios/` file mirrors one in the model's
  `conformance/`.
- **Instructions are data.** No instruction text exists in the engine and no
  engine behaviour depends on an instruction's wording (119); a session is given
  the versions in force when it starts and its inputs are enumerable and closed
  (88, 89); a test renders the exact prompt a scenario would produce without
  starting a session (90, 124); changing one is a chore and is versioned so a
  session started before it and one started after can be told apart (123).
- **Coexistence.** The new flywheel runs beside the current one against the same
  organization with a disjoint, explicit scope of objects (96).

### What stays faked until phase 2

- **Sessions.** The exits are real as a contract — done with deliverables,
  blocked on a question, offering a finding, offering a chore, stalled (65) —
  and the machinery decides what an exit means (66), but no agent runs them. The
  runner that charges a session on a multiplexer pane is A.17 and lands in phase
  2 (171). Phase 1 plays every session from a script through the same binding a
  real runner will replace (93). Where phase 1 needs curation without a runner
  it has one: a person writing the same records by hand is curation (110), and
  the enumerator half of every adapter runs unattended while turning a capture
  into signals stays a judgment that never runs unattended (115).
- **Git for lines, places, merges and landing.** Real git in phase 1 is the
  state repository as the control plane (160–167), the blueprints repository,
  and the machinery's own writes under its prefix (203, 204, 205). Everything in
  A.5 that changes what a branch or a working place is — creating a line,
  preparing or removing a place, merging into the bolt, landing (42, 49–55) —
  stays a fact in the store until phase 2, because there is no construction to
  perform it on.
- **Construction itself.** No bolt, unit, work item, stage or unit type runs.
  Planning (28), the unit proposal document (36, 17), the type catalogue (37,
  57), pull-request landing (53, A.18) and the flywheel instrument (214) are
  phase 2. Their machines are loaded and their decision kinds derive, so a
  scenario can walk them against the stand-in, but no host declares them (149).
- **The tracker profile.** C.1 is a second implementation of the same contract
  and joins in phase 2 for willdan's board (156–159). Phase 1 admits it by
  construction and does not build it: the machines do not change between
  profiles (139), and a profile is admitted when its binding is complete and the
  conformance suite passes unchanged (140, 168).
- **Claims, the ledger and the context map.** A.14 and the map (195, 198–202)
  are phase 3. Phase 1 keeps only what stops them being foreclosed: a repository
  record holds git details alone and never a hand-declared kind, capability or
  scope (199), and no claim is planned against (98).

## Capabilities

### New Capabilities

- `engine/definitions-and-tick`: machines and profiles loaded from
  `definitions/`; guards, regions, submachines, effects with proofs, idempotent
  repeat; the tick as the only clock; the engine/domain line (83–87, 138–140,
  75–78, 231).
- `engine/plan`: decision derivation from active states, the register and its
  numbers, grouping and folding, the decision kinds, the tail and the sinks'
  marks (7–19).
- `control-plane/contract`: the eight operations and the five guarantees as one
  profile-neutral interface every caller goes through (125–137).
- `control-plane/git-only-profile`: the complete binding of that contract to the
  state repository — the layout, the commit per effect, the push as
  compare-and-swap, leases and heartbeats, the response commit, notify and its
  bound, and the committed status page (160–167, 139, 140).
- `tools/tool-server`: the tool catalogue — one tool per operator operation,
  arguments by object id — served locally over HTTP and in the model context
  protocol's shape, as the single write path for every caller (193, 194, 4, 12).
- `surfaces/plan-page`: the page sink — the plan, the capture box, the status
  view, the object forms, the elaboration surface — served on the operator's
  private network and working on a phone (19, 141–146, 155, 209, 210).
- `surfaces/chat-sink`: one chat sink carrying the same decisions and numbers
  one line each with a link to the page, the numbered reply grammar, and
  notification routing by kind (18, 82, 152–155, 236).
- `signals/capture-and-curation`: captures, signals, moves and their
  consequences; the shipped adapters' enumerator half; curation's records and
  its cadence (106–118, 215).
- `organization/bootstrap`: the organization machine, `flywheel init`, the
  blueprints and state repositories, the GitHub App, the repository record, and
  the versioned template set (204–208, 203).
- `hosts/ownership`: the host object, its declaration, its heartbeat, leases and
  takeover, the intermittent laptop's away window, and the disconnected host
  (147–151, 150a, 165).
- `observability/run-record`: every write with its reason and evidence, expected
  against delivered, machinery problems reported and never filed as work
  (79–82, 167).
- `scenarios/conformance`: scenarios as data beside the definitions, the
  stand-in control plane and the scripted session binding, the rendered trace,
  the run against the git-only profile in a local bare repository, and the
  mirror of the model's `conformance/` (84, 92–95, 168).

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
  profile-neutral and the machines are byte-identical between bindings (139),
  so phase 2's tracker profile is a second implementation of the same interface,
  admitted when its binding is complete and the conformance suite passes
  unchanged (140, 168–170). Parts A and B do not change to admit it (Part C).
- **The tool server is the one write path.** Because the page's controls, the
  chat, the machinery and — later — the dispatch agent and a member's own client
  all call the same catalogue (193), phase 4's interpreter and phase 5's remote
  MCP endpoint are new clients of an existing server, not a second write path
  (194, 291, 293). Phase 1 therefore serves the catalogue in both shapes it will
  need: over HTTP for the page, and over stdio/in-process for sessions (291).
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
- **The page's data is a projection.** The status view is written from state and
  never read as truth (142, 76), so phase 5's page projection object — the
  status view and the rail as data with each member's sink and mark — is the
  same write to a different place (291, 298).
- **Same bytes everywhere.** No behaviour is gated at build time and no tier is
  compiled in; a hosted host differs from a laptop only in what its manifest
  binds — a tier, an identity kind and a router (299). Nothing that assumes a
  multi-tenant service belongs in this repository (296, AGENTS.md).
- **The contract is the only door.** The data plane touches nothing but the
  eight operations (125), so phase 5's tenancy and encryption (A.33) apply to
  one write path, and a control plane built to the invocation contract by anyone
  else drives this binary through 297's two modes and 298's five shapes and
  nothing more (299).

## Impact

- **Crates.** The four crates named in AGENTS.md grow toward the model's crate
  boundary (model.md §13) by addition, not widening: `flywheel-atoms` (the
  evidence and effect registries generated from `atoms.yaml`, the `ControlPlane`,
  `World` and `Sessions` traits), `flywheel-domain` (the embedded machines, the
  recutils reader and writer, the work order renderer), `flywheel-world-host`
  (git and the local router), `flywheel-cp-git` (the `ControlPlane` over the
  state repository), and `flywheel-surface` extended with the chat sink and the
  tool server. `flywheel-engine` keeps no string from `atoms.yaml`, checked by
  grep (86, I13).
- **Definitions.** No hand edits. Any clause phase 1 needs that the requirements
  do not have is proposed to `blueprints` first (AGENTS.md).
- **External systems.** One git host, depended on for exactly what section 9
  grants it — one update to a branch at a time, rejection of a stale base, and a
  call to a URL when a branch moves — and for nothing else (160, 162, 166). One
  GitHub App as the connection, its key placed by the operator (207, 207a). One
  chat channel that reaches the operator's phone, and a served page on the
  operator's private network (155, 191). Secrets are read from where the
  operator put them and never appear in configuration or code (204, 207).
- **The prototype.** `scenarios/plan-mockup.yaml` and the stand-in store stay as
  the test path (93); the stand-in stops being the only path.
- **Not touched.** The existing flywheel keeps running willdan unmodified until
  phase 2 (96, roadmap).
- **Gate.** `cargo test`, with a scenario under `scenarios/` mirroring one in
  the model's `conformance/` as the acceptance for each capability (94,
  AGENTS.md), run against the stand-in and against a local bare state repository
  with no network (92, 168).
