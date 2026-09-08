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
store with a real state store behind a trait, and narrows the faking to two
seams instead of three.

**Two words.** Part B is the **state store** (125, §3 vocabulary). The **control
plane** is the service side of A.37 and appears here only where phase 5 does.

## Goals / Non-Goals

**Goals:**

- One `flywheel` binary that runs a real loop for one organization on one laptop:
  fetch, tick, effects, decisions, delivery, response, commit (125–137, 160–167).
- The seams where phases 2 to 5 attach are traits with one implementation each in
  phase 1, not branches in the code (139, 140, 299).
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
| `flywheel-engine` | the loader, the guard algebra, regions and submachines, `plan_tick` (pure, no IO), decision derivation and the register, proofs and effect ids, the five engine machines |
| `flywheel-atoms` | the evidence and effect name registries generated from `atoms.yaml`; the `StateStore`, `World` and `Sessions` traits; the scenario file types |
| `flywheel-domain` | the shipped machines embedded, the organization's type catalogue loader, the recutils reader and writer, the work order renderer |
| `flywheel-world-host` | `World` over git and the local router; `profiles/host.yaml` is its specification |
| `flywheel-store-git` | `StateStore` over the state repository; `profiles/git-only.yaml` is its specification |
| `flywheel-surface` | the sinks (page, chat), the tool catalogue and its HTTP server, the reply grammar; `profiles/surfaces.yaml` |
| `flywheel-scenario` | the stand-in `StateStore` and `World`, the scripted `Sessions`, the conformance runner, the trace renderer |
| `flywheel` | the binary: `init`, `host`, `scenario`, `capture`, `exit`, `offer`, `note`, `refuse` |

`flywheel-engine` holds no string from `atoms.yaml` and no name from the
requirements' section 3, checked by a grep test in CI (86, I13). Its tests run
over the toy `lamp` machine, which shares no atom with the flywheel (model.md
§2.5).

*Alternative considered:* keep AGENTS.md's four crates and add modules. Rejected
— AGENTS.md says add a crate rather than widen one, and the trait registry must
be importable by the store and world crates without dragging in the engine's
internals. Adding the crates now also means the phase-2 tracker is one new crate
beside `flywheel-store-git` rather than a refactor.

### D2. Shipped definitions are embedded; the organization's types are read from the blueprints

Two sources, one rule each:

- The machines, profiles, schemas, instructions and skills the flywheel ships
  are compiled into the binary with `include_dir` as core machines an
  organization never edits, each carrying its version and named by the release's
  set version, which initialization and creation record (223, 224, 83, 208). This is what
  makes "same bytes everywhere" checkable (299) and what lets a host prove which
  set it ran.
- The organization's own unit and elaboration types, and its packages, are read
  from the blueprints repository at the shared line, so a type composed of
  existing atoms is added with no code change and no host is rebuilt for one
  (57, 85, 228).

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

So `flywheel-store-git` implements the six record operations plus `notify`,
`present`, `receive` and `status`, and inherits the rest. That is the whole
reason the phase-2 tracker crate is small and the machines do not move between
profiles (139). A binding that leaves a name unbound is not a profile, and
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
  (`git-only.yaml` `records.put`). Object writes race only on main's head, never
  on content, because each touches one file and the lease holds.
- **Repeat = no-op**: a response file is named by its delivery id, so a second
  delivery writes the same bytes (137).

*Alternative considered:* batch a tick's writes into one commit. Rejected — 127
wants per-effect identity and 135's atomicity is per write; a batched commit
makes a partial retry ambiguous and destroys the audit grain 167 asks for.

### D5. Leases and heartbeats are orphan branches, never files on main

`lease/<object id>` and `host/<host id>` each hold one orphan commit whose tree
is the record; renewal replaces it with `--force-with-lease`, and the old commit
becomes unreachable (`git-only.yaml` `layout.leases`, `layout.hosts`; model.md
§4.2). The push *is* the compare-and-swap: take with expected-old zero (or the
expired holder's commit), renew with expected-old own commit, release by
deleting the branch. Two hosts cannot both land a lease commit on the same
expected base (128, 134, 162, I15).

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
memory decides anything (75, I14, 217a).

### D8. The faked seam is exactly two traits, and the exit command is not one of them

| trait | phase 1 | phase 2 |
|---|---|---|
| `StateStore` | real (`flywheel-store-git`) | + `flywheel-store-tracker` |
| `World` — repositories, the manifest, the local router | real | unchanged |
| `World` — places, lines, merges, landings | store facts | `wt` and git |

One rule follows from that table, and the proposal states it as the phase-1
scope of construction: every shipped machine ticks, unit types among them (37,
223), and the effects of 42 are store facts, so a bolt, a unit, a work item and
a stage move through their states while nothing they do reaches a repository.
| `Sessions` | the scripted stand-in | herdr panes and Claude Code |

The seam is deliberately *not* at the store, which is where the prototype puts
it. Faking the store proves nothing about durability, single-writer or the
response-once guarantee, which are the whole point of the phase.

The one thing that must be real on the session side is the exit command. A
session reports its exit through a command the machinery provides, which writes
to the state store (67), and the stand-in plays every scripted exit by running
that same command (`profiles/sessions-stand-in.yaml`, 93). So `flywheel exit |
offer | note | refuse` is built in phase 1 and writes through
`StateStore::append`; the scenarios assert the command's writes, never the
script's internals. When the real runner arrives in phase 2 the command does not
change.

### D9. One tool catalogue, one transport in phase 1

The catalogue of `profiles/surfaces.yaml` `tools:` is one module in
`flywheel-surface`: one function per tool, arguments by object id, each call
recorded once as an op-response carrying the tool and who invoked it (193, 153).
Phase 1 gives it one transport, HTTP, which the page calls. The chat's numbered
reply grammar (`yes 412`, `421: <text>`) is the `answer` tool and nothing else
(194). The machinery's own commands call the same functions in-process. No
caller has an operation the others lack (193).

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

Instead: one organization, one operator, named in the manifest; the page is
reached over the operator's private network through the local router (191, 46);
every response records that name as `given_by` with `given_at` (153). Because
`given_by` is a field of the op-response record from the first commit, phase 4's
sign-in fills it rather than changing the record's shape.

*Alternative considered:* omit identity until there is a sign-in to justify it.
Rejected — 153 is a phase-1 clause, and retrofitting the field would rewrite
every response already committed.

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
  address with the organization in the path, opening that object in the dock
  with its answer controls in reach; a link to a host that is away says so (308,
  205a, 150a).
- A decision reaches the phone through the chat sink's own notification with
  whatever controls the platform provides; the page pushes nothing (309).

*Alternative considered:* a client-side application over a JSON API. Rejected by
310 (no client state a reload loses, one request, no external dependency) and by
15 (no rendering stored).

### D12. `status.html` is committed; the plan page is not

`render_status` writes the status projection from `list` and `get` alone and
commits `status.html` on `main`, stating the commit and time it is as of; any
running host serves it, and with no host running the operator reads the
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
run against the local bare repository on one laptop, which is what proves the
single-writer guarantee the whole profile rests on (134, 162, I15).

Scenarios carrying an operator's response also run at a 390px viewport (314).

## Risks / Trade-offs

- **git as the database: latency and history growth.** → `main` grows by one
  small commit per state change, on the order of a few hundred a day, and leases
  and heartbeats never touch it (D5, model.md §12.15). The willdan week is the
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

## Migration Plan

There is nothing to migrate: phase 1 has no prior user and no prior data, and
the existing flywheel is untouched (96).

The order below is chosen so each step is provable before the next depends on it:

1. **Crates and traits** (D1, D3). `flywheel-atoms` with the three traits;
   `flywheel-engine` unchanged behind them; the grep test for I13.
2. **Stand-in parity.** `flywheel-scenario` implements the traits over today's
   in-memory store; `scenarios/plan-mockup.yaml` and the existing tests pass
   unchanged. Nothing durable yet, nothing regressed.
3. **`flywheel-store-git`** (D4, D5, D6) against a local bare repository. The
   thirteen `contract/` files go green over the `lamp` machine — this is the
   step that admits the profile (168).
4. **Definitions embedded** (D2) with the parity test and the set version.
5. **`flywheel init` and `flywheel host join`** (204, 205, 205a, 207, 208), with
   `host doctor` refusing a hand-made layout.
6. **The tick loop as a host** (D7), the run record, `status.html` (D12).
7. **The surfaces**: the tool catalogue and its HTTP transport (D9), the page
   (D11), the chat sink and the reply grammar, notification routing by kind (82).
8. **The adapters** (D13) and the signal and move records.
9. **The twenty-one scenarios** (proposal.md — The acceptance), including the two-
   host runs and the 390px pass (D15).
10. **The willdan week** on real work, which is what ends the phase (roadmap,
    phase gates).

**Rollback.** The state repository is the only durable artifact and it is files
in git. A host that misbehaves is stopped; the state is read, and if necessary
edited, as files — which is itself a response (3, S19). There is no schema to
migrate back and no service to drain.

## Open Questions

- **Engine window defaults on an intermittent laptop.** Host stale 5m, gone 30m,
  lease expiry 24h, register retention 30d are the release's literals and are
  read from `flywheel.yaml` `engine:` at load (`record-derived.yaml`
  `engine_windows`). Whether a laptop wants different ones is answered by the
  willdan week and is a manifest change, not a code change.
- **How often `status.html` is rewritten.** Every tick where state moved is the
  simple rule; a cadence may be cheaper once the commit rate is known. Either way
  it states its as-of point (145), so the choice is invisible to every clause.
