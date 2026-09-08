# Review: the-loop specs

**Verdict: accept with fixes.** The twelve capabilities are the proposal's twelve, the format validates, every requirement traces to a clause or a decision, the vocabulary is the requirements', "control plane" appears nowhere, and all thirteen `contract/` files and all twenty-one scenarios have a spec scenario that names them. What must change before tasks: three spec scenarios say a found effect id means the effect is not performed, while `contract/write-effect.yaml` asserts it is performed again and only the write is a no-op (finding 1); the two granted exceptions 93b and 253a have no requirement (2); the committed status file that makes S20 observable is stated nowhere (3); five `binding.yaml` assertions are not carried (4); and ten scenario `expect` assertions have no spec THEN (6). The rest is testability wording, four phase-2 slips, and duplicate statements of one behaviour in two specs.

`openspec validate the-loop` and `openspec validate the-loop --strict` both print `Change 'the-loop' is valid` (exit 0). Note the CLI takes the change name as a positional argument; `--change` is not an option.

## Findings, ranked

### 1. Three specs say a known effect id skips the act; the contract file says the act repeats and the write is the no-op

- **Where:** engine/definitions-and-tick lines 80–84 ("A repeated effect writes nothing": *the effect is not performed, no write is made*); state-store/contract lines 58–61; state-store/git-only-profile lines 35–38 ("no commit is made"); design D4 line 160–161 ("found, it does nothing").
- **Clause or decision:** 73, 127; `contract/write-effect.yaml` — step 2 removes the proof from the world, then `effects: [{do: light, count: 2}]`, `writes_with_effect_id: 2`, `distinct_effect_ids: 1`, `second_reported_as_write: false`.
- **Why:** The suite separates two rules the spec fuses. 73: an act is performed whenever its proof is absent (the lamp is dark again, so `light` runs again). 127: the *write* carrying the effect's identity changes nothing the second time and is not reported as a second write. As written, the definitions-and-tick scenario asserts `light` count 1 and fails the file; the git-only scenario ("no commit is made") is the same conflict. AGENTS.md makes the model the source, so the spec yields, and D4's sentence needs the same correction.
- **Fix:** definitions-and-tick, requirement at line 66, keep "perform only when that proof is absent"; replace the scenario at 80–84 with: *WHEN the proof of a performed effect is lost from the world and the next tick performs it again — THEN the act is performed a second time, the write it makes carries the same identity, the state store changes nothing for it, and the run record reports one write and not two (73, 127).* contract 58–61 and git-only 35–38: change "the effect is not performed / no commit is made" to "the write with that identity changes nothing and no second write is reported; whether the act is performed again is the proof's question (73)". Tell the design owner D4 line 160 must read the same way.

### 2. 93b and 253a are granted, designed, and specified nowhere

- **Where:** no spec cites 93b, 253a or 236a (grep over `specs/**`); design D8 lines 298–309 and "The three clauses" 596–612; D10 lines 372–381.
- **Clause or decision:** 93b, 253a, 236a, 153; `profiles/sessions.yaml` `runners.operator`; `profiles/surfaces.yaml` `account.local`.
- **Ruling, 93b → hosts/ownership.** 93b is a host's manifest declaration (`flywheel.yaml hosts.<host>.runners`), the same kind of thing as the declaration of 149 that opens that spec, and the willdan week's real host runs it with no stand-in in the loop. scenarios/conformance is wrong for it: 93 admits one fake, the session stand-in, and 93b is explicitly not a fake — sessions.yaml says "the engine cannot tell them apart". definitions-and-tick would be defensible beside 93a, but 93a is about effects the engine performs and 93b about who runs a session, which is the host's business.
- **Text to add (hosts/ownership):**

  > ### Requirement: A host may declare the operator as its session binding
  >
  > A host MAY declare the operator as its session binding in the manifest, and a host so bound SHALL start no agent (93b, 69). Under it the machinery SHALL charge a session as it always does — a place prepared, a work order rendered, the session recorded — and the plan and the status view SHALL show the session as the operator's to run (93b, 89). The operator SHALL report through the same command a session reports through, and the exits, offers and refusals SHALL be the same records, so nothing downstream tells the two apart (93b, 67). A session charged this way SHALL be a with-operator session for every rule that turns on the type, and the machinery's own sessions, curation among them, SHALL be the operator's under the same rule (93b, 25, 110).
  >
  > #### Scenario: An approved elaboration is the operator's to run
  > - **WHEN** an elaboration is approved on a host whose manifest names the operator as its session binding
  > - **THEN** the place is prepared and the work order rendered, the session record names its place and work order, the plan shows it as the operator's to run, no agent process is started, and the session reads present and working until the operator reports (93b, 89)
  >
  > #### Scenario: The operator's exit is a session's exit
  > - **WHEN** the operator runs the exit command with deliverables for that session
  > - **THEN** the object's thread carries the same exit record a scripted exit would write, and the elaboration advances as after any session's exit (93b, 67)
  >
  > #### Scenario: The with-operator rules apply and the binding is recorded
  > - **WHEN** a host so bound starts and a session it charged goes idle for a day
  > - **THEN** the run record names the operator binding, no finish-or-keep is offered, and the session ends only by the operator's dictation (93b, 25)

- **Ruling, 253a → surfaces/plan-page.** It is a rule about serving the page, its three observables are HTTP responses and a response record's `given_by`, and the requirement at plan-page 100–105 already owns the private-network address it turns on. The writer's placement is right.
- **Text to add (surfaces/plan-page):**

  > ### Requirement: A single-operator host on a private network serves the page unsigned-in, and every response names that operator
  >
  > Until the organization's operators list holds more than one entry, a self-managed host MAY serve the page on the operator's private network with no sign-in (253a). The single entry SHALL be the identity every response records as given by, with when (253a, 153, 236a). The host SHALL refuse to serve unsigned-in as soon as a second operator is listed or the page is reached at any address but that network's (253a). The exception SHALL close when the account item exists (253a, 233).
  >
  > #### Scenario: A response names the manifest's operator
  > - **WHEN** the operator answers a decision on the unsigned-in page
  > - **THEN** the response record's given-by field is the operators list's single entry and its given-at is set (153, 236a, 253a)
  >
  > #### Scenario: A second operator closes the exception
  > - **WHEN** a second entry is added to the operators list and the page is requested unsigned-in
  > - **THEN** the host refuses to serve it and says why (253a)
  >
  > #### Scenario: An address off the private network is refused
  > - **WHEN** the page is requested at an address that is not the host's private-network address
  > - **THEN** the host refuses to serve it unsigned-in (253a)

  Also add 236a to chat-sink 25–32 or tool-server 23–26 so `given_by` on a chat reply is the same entry.

### 3. The committed status file is stated nowhere, so S20's THEN has no observer

- **Where:** git-only 152–162 (S20: "it shows the state as of the last landed commit"); run-record 98–108, 110–113; contract 124–126.
- **Clause or decision:** 132, 145; D12 (`status.html` committed on `main` by the plan's lease holder); `contract/status.yaml` and `S20.yaml` `status.source: "status.html on main"`, `as_of_is_last_write`.
- **Why:** With no host running nothing renders, so "it renders" is observable only because the view is a file on the shared line the runner can read from the bare repository. Run-record 110–113 names the plan's lease holder as writer but not what it writes or where. The plan page's "never committed" is stated (git-only 164–167); its counterpart is not.
- **Fix:** git-only requirement at 152–156: add *The status view SHALL be a file committed on the shared line, rebuilt by the holder of the plan's lease from list and read alone, stating the commit and the time it is as of; any running host SHALL serve it and, with none running, the operator reads the committed file (132, 145, 146).* S20 THEN: "the committed file on the shared line is readable from the state repository alone, states the commit it is as of, and that commit is the last that landed".

### 4. Five `binding.yaml` assertions are not carried

- **Where:** state-store/contract 128–144; scenarios/conformance 70–82.
- **Clause or decision:** 139, 140, 168; `contract/binding.yaml` `then.state_store`.
- **Why:** The spec refuses a binding that leaves an evidence or effect name unbound (137–139). The file also asserts `binding_names_no_evidence_outside_atoms`, `guarantee_mechanisms_named_for: [durable, single_writer, atomic, derivable, response_once]`, `profile_rejected_when_a_guarantee_is_missing`, `operations_used_by_engine` (exactly the eight), and `machine_files_hash_equals_repository`. The conformance spec records the hash (81–82) but never compares it to `definitions/`, which is D2's parity test.
- **Fix:** contract requirement 128–135, add: *A profile SHALL name the mechanism behind each of the five guarantees, and one that names none for a guarantee SHALL be refused; a binding SHALL name no evidence outside the atoms file; the operations the engine uses SHALL be exactly the eight of the contract (139, 140).* Add one scenario: WHEN a profile names no mechanism for one guarantee THEN it is refused and the guarantee is named. conformance 78–82: "their hash is recorded **and equals the hash of the definitions directory the binary was built from** (168, D2)".

### 5. Phase-1 clauses the proposal calls real with no requirement

- **Where:** grep over `specs/**` for the numbers.
- **Clause or decision:** 21, 27, 62, 65, 71 (I5), 96 (D14), 191 (D10a), 221 (D9), 88, 89, 91, 119, 123 (A.11, "real" per proposal table line 285), 76, 98 and 199 (proposal 336–339, foreclosure), D6's in-process notify, D7's sweep.
- **Why and fix, each one line:**
  - **21** (one elaboration awaiting approval; new material joins it) → engine/plan, beside 51–54: *WHEN a second elaboration is proposed on an intent with one awaiting approval THEN it joins that proposal and one decision stands (21).*
  - **27** (type chosen at proposal, correctable by the response) → engine/plan: *WHEN the operator answers a proposed elaboration naming another type THEN it is approved under that type and the record says so (27).*
  - **62 and 71** → engine/plan S04 (see finding 6).
  - **65** (one job, one place, bounded goal, fixed exits) → hosts/ownership under the 93b requirement, or conformance 38–47: name the five exits and that a report outside them is refused (65, 66).
  - **96** (coexistence: a separate state repository, D14) → organization/bootstrap: *The new flywheel's objects SHALL live in its own state repository and it SHALL touch nothing of the existing flywheel's (96).*
  - **191** (the private-network router names the host, D10a) → plan-page 100–105 cites 155, 205a, 46 but never 191; add "the manifest names the router per host, and phase 1's host address is the private-network router's name for the host (191, D10a); a link never names a localhost port".
  - **221** (`remove <organization>` by dictation, D9 lines 355–359) → organization/bootstrap: *WHEN the operator dictates the organization's removal THEN its sessions end, its places are removed, its state is archived, its repositories stay on disk, and its numbers are never reused (221, 15, 4).*
  - **88, 89, 119, 123** → conformance 59–63 cites 90 and 124 only; add to that requirement: *a session SHALL be given the versions in force when it starts, its inputs SHALL be the schema instruction, the type skill, the work order and the change's artifacts and nothing else, no instruction text SHALL exist in the engine, and each instruction SHALL be versioned so sessions before and after a change can be told apart (88, 89, 119, 123, 91).* The grep scenario at definitions-and-tick 55–58 can carry 119.
  - **76 and 199** → run-record 66–75 cites 142 for projection; add 76 (one source of truth per state). 199 → bootstrap 38–45: *a repository record SHALL hold its git details alone and never a hand-declared kind, capability or scope (199).*
  - **D6 in-process notify** (page response, chat message, exit command notify at once) is uncited; add to git-only 114–120: *A cause local to the host — a page response, a chat message, a session's exit command — SHALL notify in-process at once and not wait for the poll (130, D6).* State the bound: **30 seconds** (`contract/notify.yaml` `notify_latency_bound: 30s`; git-only 122–127 says "the stated bound" without stating it).
  - **D7 sweep**: contract 101–104 covers the never-notified host; state the interval (60 seconds) somewhere the runner can advance the clock against.

### 6. Scenario `expect` assertions no spec THEN carries

- **Where / file / missing assertion / fix:**
  - **S04** → engine/plan 40–44. Missing `records.document`, `state_store.finding_record_holds_text: false`, `session_interrupted: false`, `decisions.tail: [dropped]`, `states.research-1: working`. Add: "the record points at the finding's document and never holds its text (62); the session that offered it is still working and was not interrupted (58, 71, I5); the tail shows it dropped (14)".
  - **S07** → engine/plan 51–54. Missing `effects` archive_intent, land_line, remove_line each 1, `tail: [closed]`, `no_transitions: [bolt]`. Add: "the intent is archived and its line landed and removed as recorded effects (49, 54, 93a), the tail shows it closed, and no other object moved".
  - **S13** → hosts/ownership 34–39. Missing the host-gone decision raised at the long bound and answered `takeover` (`decisions after_step 6`), `status.stale: [object, host]`, `attempt_started_by_studio: 2`, `mac_mini_ended_own_pane_on_return: true`, `sessions_running_at_end: 1`. Add: "the status view shows the object and its host stale; past the long bound the host-gone decision is raised; on takeover the other host starts a fresh attempt; the returning host ends its own session and one session runs at the end". Say whether the losing host is intermittent (150a would raise no attention line unless work waits; here a session waits, so the decision is raised).
  - **S16** → conformance 28–31. Missing `scenario_valid` (schema), `trace_written: <name>.trace.md`, `trace_lists: [ticks, guards, transitions, effects, decisions, numbers]`. Add both.
  - **S29** → hosts/ownership 105–108. Missing `merges_in_ordinal_order: true` (38) and the dependency gate (31, cited by the file, not by the spec). Add: "the third starts only when its dependencies are merged, and merges are recorded one at a time in ordinal order (31, 38, 93a)".
  - **S02** → engine/plan 46–49. Missing the restart (`after_step 8` the offer stands, session alive) and `remove_place`/`rebase_place` 0. Add "across a restart the offer still stands, the session is alive, and its place is neither removed nor rebased".
  - **S18** → git-only 95–99. Missing `status_page_as_of_after_reconnect: latest`. Add.
  - **S19** → git-only 142–145. Missing `applied_responses: [commit/<sha>]`, `processes_told: 0`. Add "the response's identity is the commit itself, and no host was told by anything but its fetch".
  - **S24** → tool-server 60–63 and signals 96–98. Missing `decisions_created_by_dictation: 0`. Add "no decision is raised for it".
  - **X01** → tool-server 55–58. Missing `presence_read_as: keystroke within the profile's window`, `records.opened_by`. Add "present means a keystroke within the profile's window (25), and the record names who opened it".
  - **X08** → tool-server 36–42. Missing `tail: [dropped]`, `merge_place: 0`. Add.
  - **S06** → definitions-and-tick 73–78. `multiplexer_refused_duplicates: 2` is reachable only because `sessions-stand-in.yaml` `start_session` "refuse[s] a second start of the same name, as herdr does"; say so: "the second and third starts are refused as duplicates of the name".
  - **`contract/lease.yaml`** `stale_after_5m` → no spec names the lease stale window; add to git-only 65–72: "a lease whose renewal is older than the stale window is shown stale".
  - **`contract/present-receive.yaml`** `response_recorded_before_transition` → chat-sink 34–38 has it; contract 73–79 should too, since the file is a contract file.

### 7. Phase-2 material in phase-1 specs

- **bootstrap 105–108** "An upgrade is proposed, not performed — a chore the operator accepts": chores are units of the chore type, phase 2 (proposal 280, 285). Keep 208's stamp; drop the chore scenario or mark it deferred to `construction`.
- **contract 90–93** "A document under review returns its annotation as the response (129, 17)": the proposal defers 17 and 36 (line 323); no phase-1 decision carries an annotatable document. Drop, or rewrite for the one document phase 1 has, a finding's (S04), whose answers are pursue or drop.
- **bootstrap 79–81** "A session in a place SHALL be given a short-lived installation token … issued into the place": under 93b there is no agent and under 93a no place on disk. Keep 207's sentence, add no scenario, and note it is proved in phase 2.
- **definitions-and-tick 135–139** "a host whose manifest names the performing binding performs the effects instead": `flywheel-workspace-host` is phase 2. Cut the clause after "records it".
- **tool-server 36–42** "kills a pane by hand": panes are phase 2. Say "ends the session by hand, outside the machinery's command, so its presence evidence goes absent" (4). X08's step is `herdr agent stop`; the stand-in plays it as `session.pane: absent`.
- **bootstrap 43, 51** "worktrees only for places and the operator's bolt places": 44's bolt places are phase 2; under 93a no worktree exists. The scenario's "no other worktree exists" is fine; the requirement should say "makes no worktree while the line-and-place effects are recorded (93a)".
- **signals 55–58 / S08 `records.challenges: [providers/one-writer@3]`**: claims are phase 3. The spec should say a challenge move records the claim's name as text and its consequence on verdicts (116, 101) is not performed in this phase.

### 8. THENs the runner cannot observe

- **definitions-and-tick 117–120** "no timer, cron or scheduler of the machinery's own fires it": a property of the code, not of a run. Rewrite: WHEN the clock is advanced past the interval with no tick THEN nothing happens; WHEN the next tick runs THEN it fires once.
- **contract 18–21** "it does so through one of those operations and through nothing else": not observable in a scenario. Make it a build test (only the store crate depends on a git library; grep) and say so, as definitions-and-tick 55–58 does for the engine.
- **tool-server 93–96** "WHEN a further transport is added later": nothing to run in phase 1. Replace with the observable now: the in-process caller and the HTTP caller enumerate the same catalogue with the same schemas.
- **plan-page 113–116** "at no address beyond it": a negative over all addresses. Rewrite: the host binds the private-network address and the localhost port and no other, and the manifest names no publication.
- **chat-sink 72–75** "the notification the platform raises": the runner sees the posted message, not the phone's push. Assert the message posted to the channel carries the number, the platform's controls and the link.
- **chat-sink 40–43** "used as the platform provides it": assert the message carries the platform's answer controls and a numbered reply also answers it.
- **plan-page 25–29** "any control or form … each is reachable in both": give the driver a fixed list to enumerate (answer controls, capture box, mark-as-intent, the dock's back control, each kind's form).
- **signals 62–65** "no later act changes it": rewrite as "no tool in the catalogue edits a signal, and the signal's file is never rewritten in history".
- **git-only 20–25** "depends on no other service of the host": the observable is that the profile run passes against a bare repository with no API; say that.
- **signals 36–39** "An adapter runs unattended on the tick": none of the three phase-1 adapters is tick-driven (S22's is a command, S21's a bot event, the box a submission). Either name the transcript adapter as enumerating a declared directory on the tick, or make curation's cadence (110) the subject of definitions-and-tick 111–115 "A missed run is caught up once", which otherwise has no phase-1 subject either.

### 9. One behaviour stated in two specs

- **S24's revive**: tool-server 62 "the move is replaced"; signals 98 "its move is cleared". `S24.yaml` sets `signal.move: none` and transitions `dropped → unmoved`. Use "the drop move is removed and the signal is unmoved again" in both.
- **82 routing by kind**: chat-sink 99–115 and run-record 55–64 state the same requirement. Keep run-record's; have chat-sink's requirement say the chat is one of the sinks 82 routes to.
- **141 the status view's contents**: plan-page 60–68 and run-record 66–75. Keep run-record's; plan-page says the page carries the status view.
- **132/145 with nothing running**: contract 124–126, git-only 158–162, run-record 104–108. One mirror of S20 is enough; the other two can cite it.
- **148 one presenter**: hosts/ownership 72–78, chat-sink 83–93, run-record 110–113. Acceptable, since each states a different consequence, but chat-sink's "pinned by the manifest" is 148's own word and AGENTS.md's forbidden list has "pin"; the requirement's word stands, note it.

### 10. Small inventions and miscites

- definitions-and-tick 43–46: "the new version applies only to units approved after it" is not in 57 or 224; say "a unit already in flight continues under the version it recorded (57, 224)".
- engine/plan 60–62: "a decision state left and re-entered is a new decision with a new number" is model.md's register rule (line 722), not 15's text; cite it.
- tool-server 85–91 cites 291 and 293 (phase 5) for foreclosure; cite the proposal's "What must not be foreclosed" instead so the spec cites phase-1 sources only. signals 46–51 cites 216 the same way; acceptable if it stays a citation of why, not of behaviour.
- git-only 65–72 title "taken by a landing commit": 163's words; fine. The requirement should say leases and heartbeats are branches of their own (D5) so "off the shared line" is observable as `git log main` holding no renewal.

## Traceability table, the twenty-one

| scenario | spec scenario(s) | carried | see |
|---|---|---|---|
| S01 | engine/plan 97–101, 103–106; plan-page 19–23; conformance 112–116; contract 81–83 | yes | — |
| S02 | engine/plan 46–49 | partly | 6 |
| S04 | engine/plan 40–44 | partly | 6 |
| S05 | definitions-and-tick 97–101; engine/plan 17–20; 93–95 | yes | — |
| S06 | definitions-and-tick 73–78 | yes, say duplicates refused | 6 |
| S07 | engine/plan 51–54 | partly | 6 |
| S08 | signals 80–84, 108–111 | yes; `challenges` needs the phase note | 7 |
| S13 | hosts/ownership 34–39, 41–44 | partly | 6 |
| S16 | conformance 28–31 | partly | 6 |
| S17 | git-only 53–56; contract 37–40, 69–71 | yes | — |
| S18 | git-only 95–99; hosts/ownership 93–97 | partly | 6 |
| S19 | git-only 142–145 | partly | 6 |
| S20 | git-only 158–162; run-record 104–108 | no observer | 3 |
| S21 | chat-sink 52–55 | yes; add "the signal names its capture" | — |
| S22 | signals 17–20 | yes | — |
| S23 | signals 91–94 | yes | — |
| S24 | tool-server 60–63; signals 96–98 | partly, and inconsistent | 6, 9 |
| S29 | hosts/ownership 105–108 | partly | 6 |
| X01 | tool-server 55–58 | partly | 6 |
| X05 | hosts/ownership 15–23 | yes | — |
| X08 | tool-server 36–42; run-record 43–48 | partly; pane wording | 6, 7 |

`contract/`: read, list, durable, single-writer, atomic, derivable, response-once, status, notify (bound unstated), present-receive (one assertion to copy) are mirrored; write-effect is contradicted (1); lease lacks the stale window (6); binding lacks five assertions (4).

## Keeps

- The capability split is the proposal's twelve, and each Purpose is one sentence that matches its list at proposal 413–456.
- B.1 and B.2 are covered one requirement per operation and guarantee (contract 9–126); C.2 160–167 each have a requirement in git-only; D4a's disconnected rules are carried whole (git-only 85–112), D5's branches, D9's two message shapes (chat-sink 45–60), D11's page (plan-page 9–58), D12's single writer (run-record 110–113), D13's adapters (signals 27–51) and D15's second host and 390px driver (conformance 91–121).
- The vocabulary check passes: no job, task, ticket, mint or instance; "state store" throughout Part B; "control plane" nowhere; tracker appears only as its absence.
- The scenario headings name the conformance file they mirror, which is what lets tasks map `scenarios/` to `conformance/` one to one.
- The refusal path is coherent across four specs: a tool that asserts work done does not exist (tool-server 28–34), the arriving claim is unapplicable and under attention (contract 85–88), the refusal is in the run record (run-record 43–48), and the pane-gone reading is a fresh attempt (X08).
