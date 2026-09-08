# Review 1: the-loop design

**Verdict: revise.** The crate boundary, the git-only write path, the lease branches, notify, the tick, the tool catalogue and the phone section are right against the clauses they cite and against `git-only.yaml`, `surfaces.yaml` and model.md §2.1, §4.2, §13. Three things are missing or wrong at the level a spec cannot be written from: the disconnected half of C.2 (151, 165, S18) has no decision; the real `flywheel host` of the willdan week has no `Sessions` binding and no crate returning the "store facts" half of `World`, so D8's seam is not implementable as stated; and the page the phone must reach is bound to a localhost port through a router the model reserves for places' services. Below them: the store crate's work is undercounted (D3), the retry rule has no answer for the operator's own commit (D4 against S19), the grep rule of D1 cannot pass, and several bindings phase 1 needs are named nowhere (the chat platform, the 390px driver, the identity exception). Four findings from proposal-2 carry into the design and are listed, not re-argued.

## Findings, ranked

### 1. Disconnected operation is not designed

- **Where:** D4–D7 (lines 122–189), Risks (327–356), D15 line 320 (`disconnect … one`).
- **Clause:** 151, 165, 133, I14; `git-only.yaml` `guarantees.disconnected`; S18 (`disconnect:` / `reconnect:` steps, `mac_mini_commits_landed_after_reconnect`).
- **Why:** 165 has two halves. D7 takes the fetch-first half and cites 165 for it (line 182). The other half — what a host may and may not do with no route to the git host, and how it reconciles — appears in no decision. The profile states it (keep ticking owned objects up to the 24h expiry, commit locally, take no lease, start nothing, deliver to no sink; on reconnect push renewals first, a rejection means the object is lost and its local commits are discarded, then rebase and push `main`), and S18 asserts it, but the design neither adopts nor restates it, and it leaves two things the profile does not answer: what `write_effect` reports while disconnected, when 133 says only a landed commit is durable and 161 says a local commit is an intention; and what a host does on restart with unpushed local commits (I14: disk holds "what is about to be committed").
- **Fix:** a decision D4a binding `guarantees.disconnected` by name, stating the allowed and forbidden acts, the reconnect order, that a write made while disconnected is recorded in the run record as an intention until its push lands, and that unpushed local commits found at start are pushed by the same rebase-retry before the first tick, or discarded when the lease is lost.

### 2. The real host of phase 1 has no session binding and no owner for the faked half of `World`

- **Where:** D8 lines 191–211 (the table and "The seam is deliberately not at the store"); Non-Goals line 43; Goals lines 30–32 ("traits with one implementation each in phase 1, not branches"); Migration step 10.
- **Clause:** 93, 24, 65–67, 69, 110; `profiles/sessions-stand-in.yaml` line 11 ("`flywheel host` never loads it"); `conformance/README.md` ("the real profiles run … a `World` whose git and wt calls run against sandbox repositories"); model.md §13 (`World`: one method per host effect); S01 (`prepare_place` count 1, `start_session` count 1).
- **Why:** Three gaps in one seam. (a) The willdan week runs `flywheel host`, which never loads the stand-in, so when the operator approves an elaboration (24: one session, one place) the machine reaches `placing` and calls `prepare_place` and `start_session` against nothing the design names. (b) D8 says places, lines, merges and landings are "store facts" in phase 1, but §13's `World` is one trait; a `World` that is real for repositories and the router and fake for places is either a fake inside `flywheel-world-host` or a branch, both of which the Goals forbid, and no crate in D1's table owns it. (c) 93 allows exactly one fake, the session binding, and the conformance README expects the git-only path to make real places with `wt`; faking the place half is a second fake and a departure from 93 that the design neither names nor proposes to blueprints (AGENTS.md).
- **Fix:** split `World` in `flywheel-atoms` into two traits along the line D8 already draws — repositories, the manifest and the router in one; lines, places, merges and landings in the other — each with one implementation in phase 1, the second being a recorded implementation in a named crate that `flywheel host` loads by the manifest, replaced (not branched) in phase 2. State the phase-1 `Sessions` binding of `flywheel host` outright: the operator is the session (69, 110) — `start_session` records the session and the plan shows it as the operator's to run, and the operator reports through `flywheel exit`; or the elaboration halts at `placing` under attention. Then say in one sentence that phase 1 fakes places as well as sessions, cite 93 as the clause it exceeds, and propose the phase-1 exception to blueprints.

### 3. The phone cannot reach a page on a localhost port, and D10's router citation is wrong

- **Where:** D10 lines 242–244 ("the page is reached over the operator's private network through the local router (191, 46)"); D11 lines 267–270; Goals line 35.
- **Clause:** 155, 306, 308, 309, 245, 191, 205a; `profiles/host.yaml` `router.local` (line 366: "for a place's services only — the host's page serves on its own localhost port and needs no router (245)").
- **Why:** 191's local router names a place, not the host; the host's page under it is `localhost:<port>` (245). A Discord notification on the phone carrying `http://localhost:4242/<org>/…` (308) opens nothing, so 306 and 309 fail for every decision on the willdan week. 191 gives the router that names the host on the operator's network — the private-network router, a tailnet hostname — and 155 says the page is served on that network. The design binds neither, and 46 (a place's endpoints) does not support the sentence.
- **Fix:** phase 1's host address (205a) is the private-network router's name for the host, recorded from `flywheel.yaml` `hosts.<host>.router`, with the localhost port kept for the operator at the laptop as 245 allows; every link of 308 is written at that address. Name the router kind the willdan week runs (tailnet) and cite 191 and 155, not 46.

### 4. D4's retry has no rule for the operator's own commit

- **Where:** D4 lines 132–135 ("Object writes race only on main's head, never on content, because each touches one file and the lease holds"); Risks lines 338–342.
- **Clause:** 3, 164, S19, I15, 134.
- **Why:** The operator is not a lease holder. S19 commits `objects/intent/loop-granularity/object.rec` by hand while a host may be about to write the same file; the host's rebase then conflicts on content, which is neither a push rejection nor covered by "three rejections report and re-read". The profile's "no content conflict while the lease holds" does not hold against 3.
- **Fix:** one sentence in D4: a rebase that conflicts is a loss — the local commit is discarded, the object is re-read, and the operator's commit is the response (3, 164), which is what I15 requires (one commit has seen the other).

### 5. D3 undercounts what `flywheel-store-git` implements

- **Where:** D3 lines 112–113 ("implements the six record operations plus `notify`, `present`, `receive` and `status`, and inherits the rest").
- **Clause:** 125, 127, 128, 162, 163; `git-only.yaml` `contract.lease`, `contract.write_effect`, `contract.read`.
- **Why:** `record-derived.yaml` derives evidence and effects over the six record operations; it does not derive the contract's `lease` (take with expected-old zero, renew with own commit, release by delete, the 24h expiry rule) or `write_effect`'s repeat detection (`git log --grep=<effect id>` on the fetched main), which `git-only.yaml` binds at the contract level. As written the compare-and-swap of D5 and the idempotent check of D4 belong to no method the store crate is said to implement.
- **Fix:** list the eight methods of the trait as the store crate's work, saying which are written over the six record operations (`read`, `list`, `status`) and which are contract-level git (`write_effect`, `lease`, `notify`, `present`, `receive`).

### 6. D1's grep rule cannot pass, and the rec format's home is ambiguous

- **Where:** D1 lines 70–73 ("no string from `atoms.yaml` and no name from the requirements' section 3, checked by a grep test"); line 61 (`flywheel-engine` "keeps what it has", which per AGENTS.md includes the rec format) against line 63 (the recutils reader and writer in `flywheel-domain`); lines 75–79 (the alternative's reason).
- **Clause:** 86, 87, I13; model.md §2.5 (the engine holds decision derivation, the register, the five engine machines `host`, `lease`, `response`, `plan`, `sink`); §13 (`flywheel-atoms` depends on `flywheel-engine`); `crates/flywheel-engine/src/rec.rs:90` (`id: unit/atlas/x` in a test).
- **Why:** Section 3's vocabulary includes plan, decision, response, host, lease, sink and tick, all of which the engine must name by §2.5; 86 forbids seven names (intent, elaboration, bolt, unit, work item, claim, verdict). A grep for "any section 3 name" fails on day one; a grep for 86's list also fails today on `rec.rs`. Separately, D1 says the engine "keeps what it has" while giving the recutils reader and writer to `flywheel-domain`: a move or a duplicate, not "by addition". And the alternative's reason — traits importable "without dragging in the engine's internals" — contradicts §13, where `flywheel-atoms` depends on `flywheel-engine`.
- **Fix:** state the grep as 86's seven names plus every string of `atoms.yaml`, over `src/` and `tests/`, and fix or exempt `rec.rs:90`; say whether `rec.rs` moves to `flywheel-domain` or stays as the generic record format with the domain's schema on top; give the real reason for the crates (the model's boundary, §13) and drop the false one.

### 7. `status.html` has no named writer, and its commits are left out of the count

- **Where:** D12 lines 280–283; Risks lines 329–331 ("one small commit per state change").
- **Clause:** 132, 145, 142; `machines/engine/plan.yaml` `status` region (`render_status` fires when `plan.status_current` is false); D4's "never on content".
- **Why:** Two hosts both rendering `status.html` conflict on content at every rebase. The model prevents it only because `render_status` is an effect of the `plan` object, whose lease one host holds; the design does not say so, and D4's no-conflict claim silently depends on it. Each rewrite is also a commit on `main`, so the growth estimate is roughly doubled.
- **Fix:** name the plan object's lease holder as the only writer of `status.html`; count its commits in the risk.

### 8. 314's 390px pass has no mechanism

- **Where:** D15 line 325; Goals line 38; Migration step 9.
- **Clause:** 314, section 12 ("names the real tool"); `surfaces.yaml` `phone.conformance`.
- **Why:** `flywheel scenario run` runs data over the engine and the stand-in; nothing in D15 or D1 renders a page at a viewport, taps a control, or asserts the answer recorded. Tasks cannot be written for the pass without a driver.
- **Fix:** name the driver (a headless browser under a script the runner invokes, or a crate) and what the pass asserts: the decision card's controls reachable by tap at 390px, the response recorded with `given_by` and `given_at` after a reload (310, 311, 153).

### 9. The chat sink is never named, and free text has no phase-1 handling

- **Where:** D9 lines 218–220; D11 lines 271–272; D13 line 290; Impact of the proposal names "one chat channel".
- **Clause:** 152, 155, 194, 309; section 9 (the chat is a Discord bot); `surfaces.yaml` tools comment (`serenity`), `interpreter.chat` (the dispatch agent, phase 4).
- **Why:** Specs and tasks need the platform, the bot's scopes, and the rule for a message that is neither `yes N` / `N: <text>` nor a forward, since the interpreter of 194 is phase 4. 194 forbids parsing it; the design says nothing about refusing it.
- **Fix:** name Discord and the crate; state the two message shapes phase 1 accepts (the reply grammar to `answer`, a forward to `capture`) and that any other message gets a reply saying so and writes nothing.

### 10. D10 ships an unauthenticated page against 253 without naming the exception

- **Where:** D10 lines 242–246.
- **Clause:** 253 ("there is no local-user case and no unauthenticated page"), 233, 243, 155; `surfaces.yaml` `account.local`.
- **Why:** The roadmap puts 233 in phase 4 and 243/253 in phase 5, so deferring the sign-in is defensible; but the profile the design promises to run from its definitions binds `account.local` to the device flow, and 253 forbids the page phase 1 serves. A deferral is not the same as an exception, and the manifest key that names the one operator (`operators:` first entry, 236a) is not stated.
- **Fix:** state the exception as one: phase 1 serves the page with no sign-in on the private network alone (155), `given_by` is the first entry of the manifest's `operators:` list (236a), and 233 closes it in phase 4; propose the phase-1 exception to blueprints alongside finding 2's.

### 11. Inherited from proposal-2, unchanged here

- **Units tick or they do not** (proposal-2 finding 1). Non-Goals line 43 says no construction machinery; D8 line 197 ("store facts") takes the other side, that every machine ticks with the place effects recorded. Say which, once; findings 2 and 15's S13/S17/S18 depend on it.
- **S22's adapter** (finding 3). Goals line 38 counts eighteen scenarios and D13 line 290 ships two adapters; S22 runs the meeting transcript's enumerator.
- **S13's path** (finding 5). D15 lines 321–322 run S13 "against the local bare repository"; S13 is `profiles: [all]`.
- **Embedded machines' clause** (finding 6). D2 line 87 cites 208 and 83 for the machines inside the binary; 223 and 224 are the clauses, 208 the set.

### 12. Small miscites and omissions

- D7 line 189 cites 217a (dispatch is stateless) for a host's memory; 136 and 75 are the clauses.
- D5 cites 128 and 163 but omits the expiry rule they require the model to state (`renewed_at` older than 24h, or the host decision answered `takeover`).
- D11 line 269–270: with one host, a link to a host that is away is served by nothing, so "says so" (308, 150a) holds only while a second presenter exists; say that phase 1 meets it in the two-host runs and not on the willdan week.
- Open question 2 lists four engine windows; `record-derived.yaml` `engine_windows` has six (lease stale 5m and the enrolment token 24h are missing; the token is phase 5).
- D2 line 90–93 reads "packages" from the blueprints; 228 is A.28, phase 3, and nothing in phase 1 installs one.

## Checks that pass

- **Citations, D1–D15.** Every other cited clause says what the design claims; checked against requirements.md, not the profile comments. 46 and 191 for the webhook being the operator's opt-in (D6); 127, 137, 79, 167 for the commit per effect and `applied_responses` in the same commit (D4, model.md §2.1); 128, 134, 162, I15 for the lease branch as compare-and-swap (D5, §4.2, §12.13); 130, 166 for notify by `ls-remote` and `diff --name-only` (D6); 231 and 111 for the missed run caught up under its key (D7); 193, 194, 4 for the catalogue (D9); 15, 153, 154, 205a, 209, 210, 306–311 for the page (D11, `surfaces.yaml` `phone`); 132, 141–146, S20, 77 for `status.html` (D12); 19, 107, 110–116, 203, 215, 216 for the adapters (D13); 96 (D14); the stand-in's `hosts` binding for `--hosts real` (D15).
- **Crate boundary.** D1's table matches model.md §13 crate for crate, minus the phase-2+ crates; nothing domain-shaped is added to `flywheel-engine`, and the toy `lamp` test path is the model's (§2.5).
- **Contract fidelity where decided.** Single writer, atomic per write, derivable, response-once and the audit record are each bound to the profile's mechanism and cited correctly (D4, D5); durability is right for the connected case.
- **Foreclosure.** HTTP-only with an in-process caller (D9), `given_by` from the first commit (D10), one bundle (D11), the tick as a bounded invocation (D7, 297), no tier or environment branch (Non-Goals, 299).
- **Vocabulary.** Requirements' words throughout; "job" appears only in 216's own phrase; "state store" for Part B and "control plane" only for A.37.
- **Migration order** is provable step by step, and the rollback statement (3, S19) is right.
