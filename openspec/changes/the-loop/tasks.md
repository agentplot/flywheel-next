Order follows design.md — Migration Plan: each group is provable before the next
depends on it. Spec references are `<capability>` paths under `specs/`; scenario
ids are the model's `conformance/`.

## 1. Crates and traits

- [ ] 1.1 Add `flywheel-atoms` to the workspace with the `Evidence` and `Effect` name registries generated from `definitions/atoms.yaml` at build time; verify a build failure when a name is added to the atoms file and not regenerated (D1)
- [ ] 1.2 Declare the four traits in `flywheel-atoms` — `StateStore` (eight methods), `World`, `Workspace`, `Sessions` — with no implementation; verify the crate compiles against `flywheel-engine` alone (D1, D8, `state-store/contract`)
- [ ] 1.3 Move the scenario file types into `flywheel-atoms`; verify `cargo test` still passes for `flywheel-scenario`
- [ ] 1.4 Add `flywheel-domain` holding the object envelope and the domain's record schemas over the engine's generic rec reader; verify a round-trip test parses and writes one object record (D1)
- [ ] 1.5 Rename the domain names out of `flywheel-engine`: the `rec.rs:90` fixture, its `claim` field names, the `bolt <name>` fixtures in `tests/guards.rs`, the three `eval.rs` comments, and `let (num, unit)` in the duration parser; verify `cargo test -p flywheel-engine` passes (D1)
- [ ] 1.6 Add the boundary test: grep `crates/flywheel-engine/src/**` and `tests/**` for the seven object names of 86 as whole words and for every string of `atoms.yaml`; verify it fails on a deliberately reintroduced name and passes on the tree (`engine/definitions-and-tick`, I13)

## 2. Stand-in parity

- [ ] 2.1 Implement `StateStore` in `flywheel-scenario` over today's in-memory store; verify `scenarios/plan-mockup.yaml` seeds and ticks unchanged
- [ ] 2.2 Implement `World` and `Workspace` in `flywheel-scenario` from the existing `world.rs` simulation; verify `cargo test -p flywheel-scenario` passes unchanged
- [ ] 2.3 Implement the scripted `Sessions` stand-in against the trait, playing exits through the reporting command rather than by setting evidence; verify the cascade test asserts what the command wrote (D8, 67, 93)
- [ ] 2.4 Move the binary's `seed`, `tick`, `respond`, `dictate`, `plan` and `log` paths onto the traits; verify each command behaves as before against the stand-in

## 3. The git-only state store

- [ ] 3.1 Add `flywheel-store-git` with the state repository's layout — one directory per object, the response, ask and run-record paths, the lease and host branches; verify a fresh bare repository is laid out and read back (`state-store/git-only-profile`, 160)
- [ ] 3.2 Implement `read` and `list` over the fetched shared line, with `git diff --name-only` naming what moved; verify reading twice with nothing changed returns the same evidence and writes nothing (126, 166)
- [ ] 3.3 Implement `write_effect`: one commit per effect carrying its identity, reason and evidence, with repeat detection by searching the fetched history before committing; verify a second write with the same identity makes no commit while the act itself still runs when its proof is absent (73, 127, `contract/write-effect.yaml`)
- [ ] 3.4 Implement the push with expected-old and the rebase-retry, reporting and re-reading after three rejections; verify two writers of one object cannot both land (134, 162)
- [ ] 3.5 Handle a content conflict on rebase as a loss: discard the local commit, re-read, and take the operator's commit as the response; verify against a hand commit made during a write (3, 164, I15, S19)
- [ ] 3.6 Implement `lease` as pushes to a branch per object with expected-old, plus the stale window and the 24-hour expiry; verify the lease race and the stale and expired paths against `contract/lease.yaml`
- [ ] 3.7 Implement the heartbeat branch per host; verify a month of simulated renewals adds no commit to the shared line (163, 167)
- [ ] 3.8 Implement `notify`: the 30-second bounded poll as the default and the branch-move call as an opt-in the manifest names; verify a never-notified host still converges within the bound (130, 166, `contract/notify.yaml`)
- [ ] 3.9 Implement in-process notification for local causes — a page response, a chat message, a session's report — so the object ticks at once; verify the tick happens without waiting for the poll (D6)
- [ ] 3.10 Implement `present` and `receive`, writing the response record before the transition fires and acknowledging the operator; verify against `contract/present-receive.yaml`, including the unapplicable path
- [ ] 3.11 Implement the disconnected rules of D4a — keep ticking owned objects, commit locally, take no lease, start nothing, deliver to no sink — and report a write made while disconnected as pending; verify with a scenario that cuts the route (151, 165)
- [ ] 3.12 Implement the reconnect order — renewals first, discard local commits on an object whose lease was lost, then rebase and push; verify against S18
- [ ] 3.13 Push unpushed commits found at start, before the first tick, discarding those on an object no longer held; verify a host restarted with local commits lands or drops each and infers nothing from the working tree (I14)
- [ ] 3.14 Run the thirteen `contract/` files against a local bare repository over the `lamp` machine; verify all pass — this is the step that admits the profile (168, `scenarios/conformance`)

## 4. Definitions embedded

- [ ] 4.1 Embed `definitions/` in the binary as core definitions and expose the set version; verify the binary reports the set version and the version of every core machine (223, 224)
- [ ] 4.2 Add the parity test hashing the embedded set against `definitions/` on disk; verify it fails when the directory is edited without rebuilding (D2)
- [ ] 4.3 Keep the directory load as `--definitions <dir>` for the scenario runner; verify the runner can load a directory and the host cannot (D2)
- [ ] 4.4 Load the organization's own type files from the blueprints at the shared line; verify a type composed only of existing atoms runs with no binary change and no host restart (57, 85)
- [ ] 4.5 Record the extensible machine's version on each object and hold it across a type change; verify an object in flight keeps the version its record holds (57, 224)
- [ ] 4.6 Refuse a blueprints file that would override a core machine, and report it; verify the refusal reaches the run record (223)

## 5. Bootstrap and host join

- [ ] 5.1 Implement the organization machine's effects — create or adopt the blueprints repository, create the state repository, record the App requirement, register the first host — each with its proof; verify running initialization twice writes nothing the second time (204, `organization/bootstrap`)
- [ ] 5.2 Verify a half-finished bootstrap is advanced by the next tick with no step repeated (204)
- [ ] 5.3 Raise the attention decision while the App's installation is unseen; verify it stands until the installation is present and no agent performs it (204, 207, 82)
- [ ] 5.4 Implement `flywheel host join`: bare clones of the state, the blueprints and every tracked repository under the manifest's root, one checkout per shared line, no worktree; verify a repeated join clones only what is missing (205, 93a)
- [ ] 5.5 Implement `flywheel host doctor` and run it at join and every tick; verify a hand-made path makes the host refuse to start and name the first difference (205)
- [ ] 5.6 Read the App key from where the operator placed it and mint short-lived installation tokens; verify no key or token is written to configuration, code or the state repository (207, 207a)
- [ ] 5.7 Raise the attention decision for a manifest repository the installation does not cover; verify the machinery takes nothing on it (207)
- [ ] 5.8 Stamp the set version at initialization and at repository creation; verify the stamp is present and a newer set upgrades nothing on its own (208)
- [ ] 5.9 Enforce the prefix rule: refuse and report a tracked write outside the machinery's prefix that is not the effect of a response; verify against a deliberate attempt (203)
- [ ] 5.10 Implement the organization's removal by dictation — sessions ended, places removed, state archived, repositories left on disk, counter kept; verify no number is reused after removal (221, 15, 4)
- [ ] 5.11 Hold a repository record to its git details alone; verify no kind, capability or scope can be written onto it (199)

## 6. The host loop, the workspace and the sessions

- [ ] 6.1 Implement `flywheel host` as one long-lived process with the notify-tick and the 60-second sweep, fetching before every tick; verify an `older:` guard fires on the sweep and no host decides on a stale read (D7, 165, 231)
- [ ] 6.2 Implement the host's declaration, heartbeat and lease-taking within it; verify a host takes no lease outside its declaration (149, `hosts/ownership`)
- [ ] 6.3 Raise the uncovered attention decision and clear it when the manifest covers the object; verify against X05
- [ ] 6.4 Implement the intermittent host's away window — shown away with since-when, leases standing, stall clocks paused, no attention line; verify the takeover decision appears only when work waits or the long bound passes (150a)
- [ ] 6.5 Implement takeover: a fresh attempt on the taking host, and the returning host ending its own session and reporting; verify against S13 with two host processes
- [ ] 6.6 Implement the per-host session bound with the stated waiting order; verify against S29, including merges recorded in ordinal order and nothing started twice across a restart (31, 32, 38)
- [ ] 6.7 Add `flywheel-workspace-recorded` implementing `Workspace` by writing the evidence each proof reads; verify a work item advances through place preparation with no repository touched (93a, D8)
- [ ] 6.8 Select the workspace and session bindings from the manifest and record which ran; verify the run record names them (93a, 139)
- [ ] 6.9 Add `flywheel-sessions-operator`: `start_session` records the session with its place and work order and starts no agent; verify the plan and the status view show it as the operator's to run (93b, 89)
- [ ] 6.10 Implement `flywheel exit | offer | note | refuse` writing through `StateStore::append`; verify the operator's exit produces the same record a scripted exit writes (67, 93b)
- [ ] 6.11 Refuse a report that is none of the five exits and record the refusal; verify it reaches the run record (65, 66)
- [ ] 6.12 Apply the with-operator rules to an operator-bound session: no finish-or-keep on idle, ends only by dictation; verify with a session idle for a day (93b, 25)
- [ ] 6.13 Write the run record: every write with its reason and evidence, expected beside delivered, refusals, and machinery problems never filed as work; verify against `observability/run-record` (79, 80, 81)
- [ ] 6.14 Implement `render_status` as an effect of the plan object, committing the status file on the shared line with its as-of commit and time; verify only the plan's lease holder writes it (D12, 132, 145, 148)
- [ ] 6.15 Verify S20: with no host running, the committed file is readable from the state repository alone and its as-of commit is the last that landed
- [ ] 6.16 Rewrite a drifting projection from its source on the next tick and report both values; verify against a deliberately drifted projection (77, 142)

## 7. The tool catalogue and the page

- [ ] 7.1 Implement the tool catalogue from `profiles/surfaces.yaml` as one module, one function per tool, arguments by object id; verify the in-process and HTTP callers enumerate the same tools with the same schemas (193, `tools/tool-server`)
- [ ] 7.2 Record every call once as a response carrying the tool, the object, who gave it and when; verify a call delivered twice is applied once (153, 137)
- [ ] 7.3 Omit every tool that would assert work was done, and record an arriving claim as unapplicable under attention; verify against X08 (4, 6)
- [ ] 7.4 Implement the dictation path for the undo-or-defer verbs plus service start and stop; verify each takes the same transition its decision would (4, 12)
- [ ] 7.5 Implement `open-session` and verify X01: no decision is ever raised, the record names who opened it, presence is a keystroke within the profile's window, and it ends by dictation
- [ ] 7.6 Implement `revive` and verify S24: the drop move is removed, the signal is unmoved, and no decision is raised
- [ ] 7.7 Serve the catalogue over HTTP for the page; verify a call from the page writes the same record as the in-process caller (193)
- [ ] 7.8 Bind the host's address to the private-network router's name from the manifest, keeping the localhost port for the operator at the machine; verify a link written anywhere names that address with the organization in the path and never a localhost port (191, 205a, 308, D10a)
- [ ] 7.9 Serve the page as one bundle, rendered from the register and the objects on each request, with no stored rendering and no external fetch; verify a reload after an answer shows it recorded with who gave it and when (15, 310, `surfaces/plan-page`)
- [ ] 7.10 Lay the bundle out under 760px as Decisions and Board with the dock full screen and a back control; verify the fixed control list is reachable at 390px and at the desktop viewport (306, 307, 311)
- [ ] 7.11 Give each kind its one form and open an elaboration from its intent; verify a decision is the only answerable form (209, 210)
- [ ] 7.12 Implement the capture box and the mark-as-intent control; verify text is captured verbatim as one signal of kind ask and nothing in it is parsed (19, 194)
- [ ] 7.13 Serve unsigned-in on the private network while the operators list holds one entry, recording that entry as `given_by`; verify the host refuses on a second operator and at any other address (253a, 153, 236a)
- [ ] 7.14 Bind the host to its private-network address and the localhost port and to nothing else; verify no other binding and no publication in the manifest (46, 155, 245)

## 8. The chat sink

- [ ] 8.1 Implement the Discord sink and its presenter lease; verify exactly one host delivers to it (148, `surfaces/chat-sink`)
- [ ] 8.2 Render one line per decision with its number and a link to the object, keeping each kind's form; verify the chat and the page show the same numbers (15, 18)
- [ ] 8.3 Accept the numbered reply grammar as the answer tool, expanding "yes all" into one response per decision; verify each is recorded with its own identity and applied once (11, 137, 194)
- [ ] 8.4 Accept a forwarded message as a capture; verify against S21 that one capture with its pointer and one signal naming it exist and nothing else happens
- [ ] 8.5 Reply to any other message with what the sink accepts and write nothing; verify no record is made (194)
- [ ] 8.6 Carry the platform's answer controls and the link on the posted message; verify a numbered reply also answers the same decision (309, 155)
- [ ] 8.7 Advance the sink's mark in the same write as its delivery; verify the tail is measured from each sink's own mark (14, 236)
- [ ] 8.8 Route notifications by kind to the sinks the operator sets; verify an event on a host that presents no sink reaches the routed sinks and is shown nowhere on the noticing host (82)
- [ ] 8.9 Say so when a link points at an away host; verify the message rather than a silent failure, in the two-host run (308, 150a)

## 9. The adapters, signals and curation

- [ ] 9.1 Write captures with their provenance and a pointer, keyed by source event, under the machinery's blueprints prefix; verify the same event captured twice yields one capture (111, 203, `signals/capture-and-curation`)
- [ ] 9.2 Implement the meeting-transcript enumerator; verify against S22 that a second import writes nothing and starts no session
- [ ] 9.3 Implement the chat-forward enumerator; verify against S21
- [ ] 9.4 Implement the page capture box's single ask signal; verify one capture and one signal per submission (19)
- [ ] 9.5 Write signal records with their kind, asserter, subject, assertion and verbatim excerpt, and never rewrite one; verify no tool in the catalogue edits a signal (113, 193)
- [ ] 9.6 Write move records with the signal id, target, reason and date, one standing move per signal; verify curation considers only unmoved signals (107)
- [ ] 9.7 Implement the move consequences, with a challenge recording the claim by name and version; verify the ledger consequence is not attempted in this phase (116, 101)
- [ ] 9.8 Charge curation on its cadence and on the unmoved threshold; verify a missed cadence is caught up once under the idempotent key (110, 231, 111)
- [ ] 9.9 Verify S08: twenty signals end with one move each, the joins become proposed intents, and one decision stands per proposed intent
- [ ] 9.10 Verify S23 and S24: a dropped proposed intent moves its signals and they are not re-clustered; a revived signal is unmoved and clustered again
- [ ] 9.11 Show unmoved signals by source with their age on the status view; verify none is discarded (118)

## 10. The plan, its numbers and its decisions

- [ ] 10.1 Derive the plan from active states and the register on every tick; verify the plan after a restart is identical to the one before it (7, I7, `engine/plan`)
- [ ] 10.2 Number decisions in one atomic write of the plan record, never reusing a number; verify a re-entered decision state takes a new number (15)
- [ ] 10.3 Group and fold decisions so "yes to all" is meaningful and any one can be answered alone; verify both paths (11)
- [ ] 10.4 Derive the tail from each sink's mark; verify the page's and the chat's tails differ with no rendering stored (14, 15)
- [ ] 10.5 Verify S01 end to end: a yes approves the elaboration, prepares its place, charges its session, and asks nothing further
- [ ] 10.6 Verify S02: the idle offer stands across a restart, the session is alive, and its place is neither removed nor rebased
- [ ] 10.7 Verify S04: the finding's record points at its document and never holds the text, the offering session is uninterrupted, and the tail shows it dropped (62, 71)
- [ ] 10.8 Verify S07: the close is offered, the response closes it, the intent is archived with its line landed and removed as recorded effects, and nothing else moved
- [ ] 10.9 Hold an intent to one elaboration awaiting approval and let the response correct its type; verify new material joins the standing proposal (21, 27)
- [ ] 10.10 Verify S05 and S06 against the real store: the restart changes no state and no plan; a slow start starts once with duplicates refused

## 11. The conformance suite and the phone driver

- [ ] 11.1 Implement `flywheel scenario run` over the trait set, loading the stand-ins; verify it runs a scenario file and asserts its transitions, effects and decisions (94, `scenarios/conformance`)
- [ ] 11.2 Validate every scenario against the schema and write the trace file; verify against S16 that the trace lists the ticks, guards, transitions, effects, decisions and numbers
- [ ] 11.3 Implement `--hosts real` starting a second `flywheel host` with its own root and port range, and the lose, disconnect and return steps; verify S13, S17 and S18 run on one laptop
- [ ] 11.4 Exclude scenarios asserting a real take, merge, rebase, conflict or landing when the workspace is recorded, and name the subset in the run record; verify the excluded set is S14, S32, X03 and the others the proposal defers (93a)
- [ ] 11.5 Record the machine files' hash in the run record and compare it to the definitions directory; verify a mismatch fails the run (168, D2)
- [ ] 11.6 Implement `render-order` rendering a session's prompt from a session type, an instruction version and a scenario, with no session started; verify two instruction versions render differently (88, 89, 90, 124, 123)
- [ ] 11.7 Add the headless-browser driver to `flywheel-scenario` as a test dependency; verify it opens the page at a chosen viewport and taps a control (314, D15)
- [ ] 11.8 Assert the 390px pass for every scenario carrying a response: number and answers reachable by tap, the answer posted through the tool catalogue, and a reload showing `given_by` and `given_at`; verify the same script passes at the desktop viewport (311, 310, 153, 314)
- [ ] 11.9 Run the twenty-one phase-1 scenarios on the stand-in and, for those the proposal marks git-only or both, against a local bare state repository; verify every one passes

## 12. The phase gate

- [ ] 12.1 Initialize the willdan organization on the git-only profile and join the laptop host; verify the organization reaches its bootstrapped state and the host's doctor passes (204, 205)
- [ ] 12.2 Run the loop on real work for a week — captures landing, decisions raised and answered from the phone, responses recorded, the record readable; verify the week completes with no state edited by hand (roadmap, phase gates)
- [ ] 12.3 Record what the week measured against the two open questions of design.md — the engine windows and the status file's rewrite cadence — and set them in the manifest if they need setting
- [ ] 12.4 Confirm the whole suite is green and `definitions/` is byte-identical to the model; verify `cargo test`, the conformance run and the model's `check.py`
