## Context

See proposal.md — Why. This document is how.

Three things shape every decision below.

**The model is already the design at one level.** `models/statechart/` states the
machines, the atoms, the profile bindings and the conformance suite, and
`definitions/` is its byte-for-byte mirror (83, AGENTS.md). So this design does
not re-derive machines or guards. It settles what the *binary* looks like around
them: the crate boundary, where the definitions come from at runtime, how the
git-only profile's seven operations are implemented, what is faked and at which
seam, and how the surfaces are served.

**The prototype is a seed, not a base.** The engine, the register, the tail and
the effects are real and stay (README.md). The store is one JSON file
(`state/store.json`), the world is a simulation, and `flywheel-surface` serves a
page that reads that store directly. Phase 1 keeps the engine, replaces the
store with a real state store behind a trait, and moves what stays faked out of
the test harness into named crates a real host loads (D8).

**Two words.** Part B is the **state store** (125, §3 vocabulary). The **control
plane** is the service side of A.37 and appears here only where phase 5 does.

## Goals / Non-Goals

**Goals:**

- One `flywheel` binary that runs a real loop for one organization on one laptop:
  fetch, tick, effects, decisions, delivery, response, commit (125–137, 160–167).
- The seams where phases 2 to 5 attach are four traits with one implementation
  each in phase 1, not branches in the code (139, 140, 299).
- Every durable fact is a commit on `<org>/flywheel-state`'s shared line, and the
  record is readable as files with no host running (160, 161, 132, 145).
- The page is the phone (306–311) and the tool catalogue is the one write path
  (193, 194).
- The acceptance in proposal.md — The acceptance runs green: thirteen `contract/`
  files and twenty-one scenarios, on the stand-in and against a local bare state
  repository (168, 314).

**Non-Goals:**

- Construction reaching a repository: no real line, place, merge or landing
  (A.5's 42, 49–55; phase 2). The machines that drive them still tick, with
  those effects as store facts (D8).
- A second state-store profile. The tracker is phase 2 and is designed for here
  only by keeping the contract profile-neutral (139, 140, 156–159).
- Performance work. Correctness first (section 8, non-goals).
- Identity infrastructure: no device flow, no roles, no permissions (A.29, A.32;
  phases 4 and 5).
- Any code path conditional on a tier, an environment or a hosted mode (299).

## Decisions

### D1. Take the model's crate boundary now, by addition

`flywheel-engine` keeps what it has and gains nothing domain-shaped; the atoms,
the domain, the world, the store and the surfaces become their own crates, as
model.md §13 lays them out.

| crate | holds |
|---|---|
| `flywheel-engine` | the loader, the guard algebra, regions and submachines, `plan_tick` (pure, no IO), decision derivation and the register, proofs and effect ids, the five engine machines, the generic rec reader and writer |
| `flywheel-atoms` | the evidence and effect name registries generated from `atoms.yaml`; the `StateStore`, `World`, `Workspace` and `Sessions` traits (D8); the scenario file types |
| `flywheel-domain` | the shipped machines embedded, the organization's type catalogue loader, the object envelope and the domain's record schemas, the work order renderer |
| `flywheel-world-host` | `World` over git, the manifest and the host's router; `profiles/host.yaml` is its specification |
| `flywheel-workspace-recorded` | `Workspace` as records: each line, place, merge and landing effect written as a fact in the state store (D8) |
| `flywheel-sessions-operator` | `Sessions` with the operator as the session: the work shown on the plan, reported through `flywheel exit` (D8) |
| `flywheel-store-git` | `StateStore` over the state repository; `profiles/git-only.yaml` is its specification |
| `flywheel-surface` | the sinks (page, chat), the tool catalogue and its HTTP server, the reply grammar; `profiles/surfaces.yaml` |
| `flywheel-scenario` | the stand-in `StateStore`, `World` and `Workspace`, the scripted `Sessions`, the conformance runner and its 390px driver, the trace renderer |
| `flywheel` | the binary: `init`, `host`, `scenario`, `capture`, `exit`, `offer`, `note`, `refuse` |

**The grep rule, stated so it can pass.** model.md §2.5 says the engine names
nothing from `atoms.yaml` and no name from the requirements' section 3, but
section 3 also defines plan, decision, response, host, lease, sink and tick,
which the engine must name because it owns the register, the decision
derivation and the five engine machines (§2.5). The rule the test enforces is
86's: the seven object names — intent, elaboration, bolt, unit, work item,
claim, verdict — as whole words, plus every string of `atoms.yaml`, over
`crates/flywheel-engine/src/**` and `tests/**` (86, I13). It fails today and
phase 1 fixes what it finds: the record fixture at `rec.rs:90`
(`id: unit/atlas/x`) and its `claim` field names, the `bolt <name>` answer
fixtures in `tests/guards.rs`, three comments in `eval.rs`, and the one true
collision, `let (num, unit)` in the duration parser, renamed. Its tests run over
the toy `lamp` machine, which shares no atom with the flywheel (§2.5).

**Where the rec format lives.** AGENTS.md puts the rec format in
`flywheel-engine` and model.md §13 puts "the recutils reader and writer" in
`flywheel-domain`. Phase 1 reads them together: the generic reader and writer
stay in `flywheel-engine`, where they name nothing of the domain, and
`flywheel-domain` owns the object envelope and the domain's record schemas
written on top of them. Nothing moves.

*Alternative considered:* keep AGENTS.md's four crates and add modules. Rejected
— AGENTS.md says add a crate rather than widen one, and the model's boundary
(§13) is the shape phases 2 to 5 attach to, crate for crate: the phase-2 tracker
is then one new crate beside `flywheel-store-git`, and the phase-2 runner one
beside `flywheel-sessions-operator`, rather than a refactor of either.

### D2. Shipped definitions are embedded; the organization's types are read from the blueprints

Two sources, one rule each:

- The machines, profiles, schemas, instructions and skills the flywheel ships
  are compiled into the binary with `include_dir` as core machines an
  organization never edits, each carrying its version and named by the release's
  set version, which initialization and creation record (223, 224, 83, 208). This is what
  makes "same bytes everywhere" checkable (299) and what lets a host prove which
  set it ran.
- The organization's own unit and elaboration types are read from the blueprints
  repository at the shared line, so a type composed of existing atoms is added
  with no code change and no host is rebuilt for one (57, 85). Packages are
  A.28 and phase 3; phase 1 installs none, and the loader reads the type files
  only (228).

The prototype's runtime directory load survives as `--definitions <dir>`, used by
the scenario runner and by the parity test that hashes the embedded set against
`definitions/` on disk.

*Alternative considered:* keep the runtime directory load as the only path.
Rejected — a host would need the directory shipped and kept beside it, 208's
stamped version could not be proven from the binary, and a hand-edited directory
would silently become the machine that ran.

### D3. `StateStore` is eight methods over six record operations

The operations of 125 are seven; the trait has eight methods because present and
receive are two. Under them, every evidence and effect that is a function of an
object's record and thread is stated once, in terms of six record operations —
`get`, `put`, `append`, `list`, `responses`, `leases`/`hosts`
(`profiles/record-derived.yaml`) — and is therefore the same in every profile.

`flywheel-store-git` implements all eight methods, and they divide in two:

| method | written over | how |
|---|---|---|
| `read`, `list` | the six record operations | the object files as of the fetched `origin/main`; `list` is `git ls-tree`, and `git diff --name-only` after a fetch names what moved (`contract.read`, `records.list`) |
| `status` | the six record operations | `render_status` from `list` and `get`, committed as `status.html` (D12) |
| `write_effect` | contract-level git | one commit per effect, its id in the message, repeat detection by `git log --grep=<effect id>` on the fetched main before committing (127, `contract.write_effect`) |
| `lease` | contract-level git | take, renew and release as pushes to `lease/<id>` with expected-old, and the expiry rule (128, 162, 163, `contract.lease`; D5) |
| `notify` | contract-level git | the poll or the webhook (130, 166; D6) |
| `present`, `receive` | the surfaces | the sinks of `surfaces.yaml`, with the response committed as `responses/<delivery id>.rec` (129, 164) |

What `record-derived.yaml` gives free is every *evidence and effect name* stated
over the six operations, which is why the phase-2 tracker crate is small and the
machines do not move between profiles (139). It does not give the contract's
`lease` or `write_effect`: `git-only.yaml` binds those at the contract level, and
they are the store crate's own work. A binding that leaves a name unbound is not a profile, and
`check.py` enforces it (140, `conformance/contract/binding.yaml`).

*Alternative considered:* bind each evidence name per profile. Rejected — that
is exactly the duplication `record-derived` exists to remove, and it would make
the two profiles' bindings diverge where the requirements say they must not.

### D4. One commit per effect and per state write, pushed with expected-old, rebase-retry three times

- **Effect id** = `<object>/<transition>/<proof evidence>/<evidence hash>`, in
  the commit message with the transition's reason and the evidence values the
  guard read (79, 127, 167). Before performing an effect the host looks for that
  id on the fetched main; found, it does nothing and reports nothing (127).
- **State write**: the new state, `entered_at`, counters and the consumed
  response's id in `applied_responses`, in one commit — the same commit — so a
  response takes effect exactly once whatever is delivered twice or restarts in
  between (137, I2).
- **Push**: `main` with expected-old = the fetched head. On rejection: fetch,
  rebase the one-file commit, push again; three rejections report and re-read
  (`git-only.yaml` `records.put`). Two hosts race only on main's head, never on
  content, because each write touches one file and the lease holds.
- **The operator is not a lease holder.** A commit the operator makes by hand on
  an object file is the response (3, 164, S19), and it can make a rebase
  conflict on content, which no push rejection covers. That conflict is a loss:
  the host discards its local commit, re-reads the object, and applies the
  operator's commit as the response — which is what I15 asks, that one of two
  commits on an object has seen the other.
- **Repeat = no-op**: a response file is named by its delivery id, so a second
  delivery writes the same bytes (137).

*Alternative considered:* batch a tick's writes into one commit. Rejected — 127
wants per-effect identity and 135's atomicity is per write; a batched commit
makes a partial retry ambiguous and destroys the audit grain 167 asks for.

### D4a. Disconnected operation

A laptop loses its route often, so C.2's disconnected half is a decision and not
a footnote. Phase 1 binds `git-only.yaml` `guarantees.disconnected` by name
(151, 165, S18).

- **What a disconnected host may do:** keep ticking the objects whose leases it
  holds, up to the 24-hour expiry, and commit locally.
- **What it may not do:** take a new lease, start a session for an object it
  does not hold, deliver to any sink, or expire another host's leases (151).
- **What a write reports.** A local commit is an intention, not a fact (161),
  and only a commit that reached the git host is durable (133). So
  `write_effect` returns the effect as *pending*, never as written: the run
  record carries the entry with its effect id and the note that it has not
  landed, and the machinery reports it as written only when its push lands. The
  effect's proof is unchanged — on the next fetch the effect id is found on
  `main` and the effect is not performed twice (127).
- **Reconnect order.** Lease renewals push first. A rejection means the object
  was taken over: the host ends its own panes for it, reports, and discards its
  local commits on that object. Then it rebases and pushes its `main` commits by
  the retry of D4 (165, S18).
- **Restart with unpushed commits.** The disk holds only what git already holds
  or what is about to be committed (I14), so at start, before the first tick, the
  host pushes any unpushed commits on `main` by that same retry, discarding those
  on an object whose lease it no longer holds. Nothing is inferred from the
  working tree, and no state is read from anywhere but git.

*Alternative considered:* refuse to tick while disconnected. Rejected — 151 says
a host keeps working what it already owns, and on a laptop that is most of the
week's work; the cost of the other rule is a queue of intentions, which the run
record already distinguishes.

### D5. Leases and heartbeats are orphan branches, never files on main

`lease/<object id>` and `host/<host id>` each hold one orphan commit whose tree
is the record; renewal replaces it with `--force-with-lease`, and the old commit
becomes unreachable (`git-only.yaml` `layout.leases`, `layout.hosts`; model.md
§4.2). The push *is* the compare-and-swap: take with expected-old zero (or the
expired holder's commit), renew with expected-old own commit, release by
deleting the branch. Two hosts cannot both land a lease commit on the same
expected base (128, 134, 162, I15). The expiry rule 128 and 163 require the
model to state is the profile's: a lease commit whose `renewed_at` is older than
24 hours, or the holder's `host-gone` decision answered `takeover`
(`contract.lease`); a laptop shown as away keeps its leases and raises no
takeover on its own (150a).

Keeping them off `main` is what lets 167 stand: `main`'s history is state
changes and nothing else, so history is a readable audit record even after
months of minute-by-minute renewals.

*Alternative considered:* lease files on `main`. Rejected — renewal traffic would
dominate the history and every renewal would race the head against real state
writes.

### D6. Notify: the bounded poll is the laptop's default; the webhook is opt-in

`git ls-remote origin main` every 30 seconds — one round trip, no history read —
is what a laptop runs. After a fetch, `git diff --name-only <old>..<new>` names
the object files that moved, so a host re-reads only those (130, 166). The git
host's push webhook to a host's `/hook` is faster and is the operator's choice,
because it needs an address reachable from outside the private network, and
publishing one is never the machinery's decision (46, 191). Local causes — a
page response, a chat message, a session's `flywheel exit` — notify in-process
immediately and do not wait for the poll.

Notification only shortens the wait: a host that is never notified still
converges by reading (130).

### D7. The tick is a notify-tick plus a 60-second sweep, and it fetches first

Per model.md §2.1: a notify ticks that object and its parent chain; a sweep every
60 seconds covers every scope the host has a lease or a candidate on, which is
what makes `older:` guards fire and what makes a never-notified host converge
(130). Every tick fetches and integrates the shared line first, so no host
decides on a read older than the bound and no person runs the sync by hand
(165). Every timed behaviour of the host, adapters included, is a guard on that
tick; nothing else keeps time, and a run missed while the laptop was shut is
caught up on the next tick under its idempotent key (231, 111).

This is also the shape phase 5 needs: a tick is a bounded invocation that lists,
reads, decides, writes, delivers, writes the run record and could exit (297).
Phase 1 does not exit — it is a long-lived process — but nothing it holds in
memory decides anything: every state it acts on is derivable from what `read`
and `list` return (136, 75, I14).

### D8. Four traits, one implementation each, and the exit command is real

`World` as model.md §13 states it is one trait with one method per host effect.
Phase 1 needs half of it real and half recorded, and a trait that is real for
some methods and recorded for others is either a fake inside
`flywheel-world-host` or a branch, both of which the Goals forbid. So
`flywheel-atoms` carries four traits, not three, split on exactly the line the
phase draws:

| trait | what it covers | phase 1 | phase 2 |
|---|---|---|---|
| `StateStore` | the operations of 125 | `flywheel-store-git` | + `flywheel-store-tracker` |
| `World` | repositories, the manifest, the router, the App's tokens | `flywheel-world-host` | unchanged |
| `Workspace` | lines, places, merges, landings — the effects of 42 | `flywheel-workspace-recorded` | `flywheel-workspace-host`, over `wt` and git |
| `Sessions` | starting, observing and ending a session | `flywheel-sessions-operator` | `flywheel-sessions-herdr`, panes and Claude Code |

Each has one implementation in phase 1 and is replaced, not branched, in phase
2; `flywheel host` loads the pair the manifest names
(`flywheel.yaml hosts.<host>.workspace` and `.sessions`), and the scenario
runner loads the stand-ins instead.

**`Workspace` recorded.** Every effect of 42 — create a line, prepare or remove
a place, merge into the bolt, land — is performed by writing the fact the effect's
proof reads: `place.exists`, `place.merged`, `place.absent`, `line.landed`. The
machines tick unchanged over those facts, unit types among them (37, 223), so a
bolt, a unit, a work item and a stage move through their states while nothing
they do reaches a repository. This is what the prototype already does
(README.md, `crates/flywheel-scenario/src/world.rs`); phase 1 makes it a named
crate a real host loads rather than a property of the test harness.

**`Sessions` with the operator as the session.** The willdan week runs
`flywheel host`, which never loads the stand-in
(`profiles/sessions-stand-in.yaml`), so an approved elaboration reaching
`placing` must charge a session against something real. In phase 1 that
something is the operator (69, 110): `start_session` records the session with
its place and its work order, the plan and the status view show it as the
operator's to run, and the operator does the work and reports through
`flywheel exit | offer | note | refuse` — the same command the stand-in plays
and the same one the phase-2 runner will call (67, 93). The machinery's own
sessions are the same: curation is a person writing the move records (110), and
planning does not run in phase 1. Nothing infers an exit from what is on disk
(67).

**Why the seam is not at the store.** Faking the store, which is where the
prototype puts it, proves nothing about durability, single-writer or the
response-once guarantee — the whole point of the phase. So `flywheel exit |
offer | note | refuse` is built in phase 1 and writes through
`StateStore::append` (67), whoever runs it, and the scenarios assert what the
command wrote and never the script's internals. When the phase-2 runner arrives
the command does not change.

**This exceeds 93, and phase 1 says so.** Clause 93 admits exactly one stand-in,
the session binding, and says everything the machinery owns is real. Phase 1
recording `Workspace` is a second stand-in, and the operator standing in for an
agent is a reading of 69 and 110 that 93 does not grant. Both are stated as the
phase-1 exception below, in "Proposed to blueprints", rather than assumed here
(AGENTS.md).

### D9. One tool catalogue, one transport in phase 1

The catalogue of `profiles/surfaces.yaml` `tools:` is one module in
`flywheel-surface`: one function per tool, arguments by object id, each call
recorded once as an op-response carrying the tool and who invoked it (193, 153).
Phase 1 gives it one transport, HTTP, which the page calls. The machinery's own
commands call the same functions in-process. No caller has an operation the
others lack (193).

**The chat is Discord, through `serenity`** (section 9, `surfaces.yaml` tools).
Phase 1's bot accepts exactly two shapes of message and no others, because the
interpreter that would read free text is the dispatch agent of 194 and lands in
phase 4:

- the numbered reply grammar — `yes 412`, `412: <text>`, `yes all` — which is
  the `answer` tool (194);
- a forwarded message, which is the `capture` tool (112, 215).

Anything else is answered with one reply naming those two shapes and a link to
the page, and writes nothing. The machinery does not parse it, guess at it, or
record it as a capture (194).

A tool that would assert work was done does not exist; a response that arrives
claiming one is recorded `unapplicable` and reported under attention (4).

Phase 1 builds no stdio or MCP transport: there is no session client to use it,
and by the proposal's own foreclosure argument a later transport is a transport,
not a second write path (291, 293). Phase 2 adds it beside the runner that needs
it.

Because the organization machine ticks unchanged, its `remove` dictation is in
the catalogue from day one and phase 1 performs it — sessions ended, places
removed, state archived, git repositories left on disk, numbers never reused
(4, 221, 15). Refusing a transition the loaded machine offers would be the
machinery deciding what the operator may undo.

*Alternative considered:* build the MCP shape now to prove the catalogue is
transport-neutral. Rejected as untested surface area — the in-process caller
already proves the catalogue is not HTTP-shaped.

### D10. Identity in phase 1 is the manifest's operator, and `given_by` is a field from day one

153 requires every response to record who gave it and when. The sign-in that
would vouch for that identity — GitHub's device flow, the operators list as
membership, roles and permissions — is A.29 and A.32, phases 4 and 5 (233, 243,
253). Phase 1 does not build it.

Instead: one organization, one operator, the first entry of the manifest's
`operators:` list (236a); every response records that name as `given_by` with
`given_at` (153). Because `given_by` is a field of the op-response record from
the first commit, phase 4's sign-in fills it rather than changing the record's
shape.

**This is an exception, not only a deferral.** Clause 253 says a self-managed
host signs every operator in through GitHub's device flow and that there is no
unauthenticated page, and `surfaces.yaml` `account.local` binds it. Phase 1
serves the page with no sign-in, on the operator's private network alone (155),
to one named operator. That exception is stated below in "Proposed to
blueprints"; 233 closes it in phase 4.

*Alternative considered:* omit identity until there is a sign-in to justify it.
Rejected — 153 is a phase-1 clause, and retrofitting the field would rewrite
every response already committed.

### D10a. The host's address is its private-network name, not a localhost port

The phone is the surface every decision must be answerable on (306), and it
reaches the page by the link every chat rendering and notification carries (308,
309). A link to `http://localhost:4242/<org>/…` opens nothing on a phone, so a
laptop serving only a localhost port fails 306 and 309 for every decision of the
willdan week.

Phase 1 therefore binds the host's address (205a) to the private-network router
191 names — the host's tailnet hostname, `flywheel.yaml hosts.<host>.router:
{kind: tailnet}` — and the page is served there, on the operator's private
network as 155 requires. Every link of 308 is written at that address with the
organization in the path. The localhost port stays available to the operator
sitting at the laptop, which is what 245 permits rather than requires; it is
never what a link names.

The routers of 191 bind a *place's* services, and phase 1 has no place serving
anything (D8), so this is the same manifest key doing the other half of its job:
naming the host on the operator's network. Nothing is published beyond that
network (46).

*Alternative considered:* keep the localhost port and put a public URL in the
chat. Rejected — publishing an address beyond the private network is the
operator's choice and never the machinery's (46), and the tailnet name needs no
such publication.

### D11. The page is one bundle, phone-first, rendered from state on every request

- One bundle is built and one is served, and its version is the binary's (307).
  Under 760px it is the same bundle: Decisions and Board tabs, the dock full
  screen with a back control — the desktop's metaphor at a smaller size, not a
  metaphor of its own (307).
- Rendered from the register and the objects on each request. No rendering is
  stored (15) and the page holds no client state a reload loses, so a reload
  after an answer shows the answer recorded with who gave it and when (310, 153,
  154). The bundle fetches nothing from anywhere else (310).
- Every decision shows its number and its answers as controls, one tap each;
  nothing is reachable only by hover or keyboard, and a long-form answer uses the
  platform's own keyboard (311, 15). The decision card is the only answerable
  form; every other kind keeps the form the status view gives it (209), and an
  elaboration is a surface of its own reached from its intent (210).
- Every chat line, notification and plan line carries a link at the host's
  address — its private-network name, D10a — with the organization in the path,
  opening that object in the dock with its answer controls in reach (308, 205a).
  A link to a host that is away says so rather than failing silently (308,
  150a); with one host there is nothing left to serve that message, so phase 1
  meets it in the two-host runs of D15 and not on the willdan week.
- A decision reaches the phone through the chat sink's own notification with
  whatever controls the platform provides; the page pushes nothing (309).

*Alternative considered:* a client-side application over a JSON API. Rejected by
310 (no client state a reload loses, one request, no external dependency) and by
15 (no rendering stored).

### D12. `status.html` is committed; the plan page is not

`render_status` is an effect of the `plan` object (`machines/engine/plan.yaml`,
`status` region), so the holder of the plan's lease is its only writer. That
matters for D4: `status.html` is the one file every host would otherwise write,
and it is the single-writer rule, not the one-file-per-object rule, that keeps
it from conflicting on content. It writes the status projection from `list` and
`get` alone and commits it on `main`, stating the commit and time it is as of;
any running host serves it, and with no host running the operator reads the
committed file (132, 141–146, S20). The plan page is served and never committed,
because no rendering of the plan is stored (15). Drift between a projection and
its source is rewritten from the source on the next tick and reported to the run
record with both values (77, model.md §3.3).

### D13. Three adapters, all enumerators, and the operator is curation

Phase 1 ships the page's capture box (19), the chat forward (112, 215, S21) and
the meeting transcript (111, 215, S22). Each writes one keyed capture per
source event with its provenance and a pointer to raw material that stays
outside every repository (111), under the blueprints' `flywheel/` prefix (203).
Capturing the same source event twice yields one capture (111, S22).

Turning a capture into signals is a judgment and never runs unattended (115). In
phase 1 the page's box writes its single ask signal directly, which is not a
judgment but a control (19), and everything else is the operator writing move
records by hand — a person writing the same records is curation (110). Every
signal takes exactly one standing move with a stated consequence, and only the
operator's response replaces one (107, 116).

The pull-request and issue-tracker adapters of 215 read the git host's issues and
reviews, which C.2 forbids the machinery doing; they belong to the tracker
profile in phase 2. The capture endpoint is one of dispatch's four jobs and is
phase 4 (216).

### D14. Coexistence is a separate state repository

The new flywheel's objects are the state repository's, and the old flywheel
touches nothing in it. The scope is disjoint and explicit because it is a
different repository, not a filter (96). The old flywheel keeps serving willdan
unmodified until phase 2 (roadmap).

### D15. The conformance runner starts a real second host

`flywheel scenario run [--profile git-only] <dir>` runs the suite as data
(`conformance/README.md`). A scenario's `host:` step names the host the following
steps run as and may start, lose, disconnect or return one; `--hosts real`
starts a second `flywheel host` as its own process with its own root and port
range (`profiles/sessions-stand-in.yaml` `hosts`). That is how S13, S17 and S18
run on one laptop, which is what proves the single-writer guarantee the whole
profile rests on (134, 162, I15). S17 and S18 are `git-only` scenarios; S13 is
`profiles: [all]` and runs on both paths.

**The 390px pass needs a driver.** `flywheel scenario run` runs data over the
engine and the traits; nothing in it renders a page, taps a control or reads
back a record, so 314 has no mechanism without one. Phase 1 adds a headless
browser the runner drives — one script per scenario carrying an operator's
response — asserting three things at a 390px viewport: the decision's number and
its answers are visible and reachable by tap, with nothing behind a hover or a
keyboard (311); the answer posts through the same tool the reply grammar calls
(193); and a reload shows the answer recorded with `given_by` and `given_at`
(310, 153, 154). The same script at the desktop viewport is the second half of
314. The driver is a test dependency of `flywheel-scenario` and of no shipped
crate.

## Risks / Trade-offs

- **git as the database: latency and history growth.** → `main` grows by one
  small commit per state change plus one `status.html` rewrite per tick where
  state moved (D12) — call it twice a few hundred a day — and leases and
  heartbeats never touch it (D5, model.md §12.15). The willdan week is the
  measurement. If it does not hold, the answer is the tracker profile in phase 2,
  not a change to Part A or B — which is precisely why the contract is
  profile-neutral (139, 140).
- **A 30-second poll makes the loop feel slow.** → The poll is a floor for
  *another host's* writes only. Every local cause notifies in-process at once
  (D6), and phase 1 has one host, so the operator's own actions are immediate.
- **A rebase-retry could lose a write under an unexpected conflict.** → Object
  writes touch one file each and cannot conflict on content while the lease
  holds; three rejections report and re-read rather than force
  (`git-only.yaml` `records.put`). `--force-with-lease` is used only on the lease
  and host branches, where the expected-old *is* the compare-and-swap.
- **The stand-in could drift from the real runner.** → The seam is the exit
  command, which is real and identical in both (D8, 67, 93). A scenario asserts
  what the command wrote, so a real runner passes the same assertions.
- **`definitions/` could drift from the model once it is embedded.** → It is
  never hand-edited; the model changes first, `check.py` reports every cited
  clause, then the copy (83, AGENTS.md). A build-time parity test hashes the
  embedded set against the directory, and the hash goes in the run record
  (`conformance/README.md`).
- **Phase-1 shortcuts foreclosing phases 2 to 5.** → proposal.md — What must not
  be foreclosed is the acceptance for this design; D1, D3, D8 and D9 are where
  each later phase attaches, and none of them is a branch in the code (299).
- **One operator, one laptop hides multi-host bugs.** → Mitigated by running
  S13, S17 and S18 with two real host processes (D15) rather than deferring them
  to the phase that has two machines.
- **The operator as the session (D8) hides what a real runner will hit.** → The
  seam is the exit command, identical in both, and the work order the operator
  reads is the one `prepare_place` renders (89), so phase 2 changes who runs it
  and nothing about what it says. What stays untested until phase 2 is start
  latency, the pane's presence evidence and the multiplexer's refusal of a
  duplicate name (72, 196).
- **`flywheel-workspace-recorded` could mask a real git problem.** → It is meant
  to: phase 1 has no construction to perform. The risk it leaves is that the
  merge, rebase and conflict scenarios (S14, S32, X03) are unproved until phase
  2, which is why they are deferred with that reason rather than approximated.

## Migration Plan

There is nothing to migrate: phase 1 has no prior user and no prior data, and
the existing flywheel is untouched (96).

The order below is chosen so each step is provable before the next depends on it:

1. **Crates and traits** (D1, D3, D8). `flywheel-atoms` with the four traits;
   `flywheel-engine` unchanged behind them; the grep test of D1, with the
   fixtures it names renamed.
2. **Stand-in parity.** `flywheel-scenario` implements the four traits over today's
   in-memory store; `scenarios/plan-mockup.yaml` and the existing tests pass
   unchanged. Nothing durable yet, nothing regressed.
3. **`flywheel-store-git`** (D3, D4, D4a, D5, D6) against a local bare
   repository. The thirteen `contract/` files go green over the `lamp` machine —
   this is the step that admits the profile (168).
4. **Definitions embedded** (D2) with the parity test and the set version.
5. **`flywheel init` and `flywheel host join`** (204, 205, 205a, 207, 208), with
   `host doctor` refusing a hand-made layout.
6. **The tick loop as a host** (D7), `flywheel-workspace-recorded` and
   `flywheel-sessions-operator` (D8), the run record, `status.html` (D12).
7. **The surfaces**: the tool catalogue and its HTTP transport (D9), the host's
   private-network address (D10a), the page (D11), the Discord sink and its two
   message shapes (D9), notification routing by kind (82).
8. **The adapters** (D13) and the signal and move records.
9. **The twenty-one scenarios** (proposal.md — The acceptance), including the
   two-host runs and the 390px pass with its driver (D15).
10. **The willdan week** on real work, which is what ends the phase (roadmap,
    phase gates).

**Rollback.** The state repository is the only durable artifact and it is files
in git. A host that misbehaves is stopped; the state is read, and if necessary
edited, as files — which is itself a response (3, S19). There is no schema to
migrate back and no service to drain.

## Proposed to blueprints

Three things phase 1 needs that the requirements do not grant. None is assumed
here; each is proposed to `agentplot/blueprints` as clause text, and the design
holds only if they land (AGENTS.md).

**1. A phase with no construction may record the line-and-place effects.**
Amends 93, which admits one stand-in.

> A build that performs no construction may bind the effects that change a line
> of work or a place to work in (42) to a recorded implementation, which writes
> the fact each effect's proof reads and touches no repository. The binding is
> named in the manifest like any other, the machines and their proofs are
> unchanged, and a scenario whose assertions are about a real merge, rebase,
> conflict or landing does not run against it. Everything else the machinery
> owns is real, as 93 requires.

*Why:* phase 1's gate is the loop — captures, the tick, decisions, responses,
the record — and there is no bolt to land. Without this the design must either
build the `wt` and git binding a phase early, or leave `flywheel host` calling
nothing when an elaboration reaches `placing`.

**2. The operator may be the session binding of a host that runs no agent.**
Amends 93 and reads 69 and 110 together.

> A host may declare the operator as its session binding. Under it the machinery
> charges a session as it always does — a place prepared, a work order rendered,
> the session recorded — and the plan shows the session as the operator's to
> run; the operator does the work and reports through the same command a session
> reports through (67). The exits, the offers and the refusals are the same
> records, so nothing downstream can tell the two apart, and a session so
> charged is a with-operator session for every rule that distinguishes them
> (25).

*Why:* 110 already says a person writing the records is curation and 69 already
gives the operator a session of their own; this states the general case, which
is what a phase with no runner needs.

**3. A self-managed host serving one operator may serve the page unsigned-in.**
Amends 253.

> Until an organization's manifest lists more than one operator, a host on the
> operator's own private network may serve the page with no sign-in. The single
> entry of `operators:` is the identity every response records as `given_by`
> (153, 236a), the private network is the boundary (155), and the host refuses
> to serve unsigned-in as soon as a second operator is listed or the page is
> reached at any address but the private network's.

*Why:* 253's device flow is A.32, phase 5, and 233's account item is A.26, phase
4. Phase 1 must record who answered (153) and must reach a phone (306) without
building either.

## Open Questions

- **Engine window defaults on an intermittent laptop.** The six of
  `record-derived.yaml` `engine_windows` — host stale 5m, host gone 30m, lease
  stale 5m, lease expiry 24h, register retention 30d, enrolment token 24h — are
  the release's literals and are read from `flywheel.yaml` `engine:` at load.
  The token is phase 5's and stands at its default; the other five are the
  laptop's. Whether a laptop wants different ones is answered by the
  willdan week and is a manifest change, not a code change.
- **How often `status.html` is rewritten.** Every tick where state moved is the
  simple rule; a cadence may be cheaper once the commit rate is known. Either way
  it states its as-of point (145), so the choice is invisible to every clause.
