# Review 2: the-loop proposal

**Verdict: revise.** Eleven of the twelve first-review findings are resolved against the sources, and finding 9's ruling goes to the proposal, not the review. What remains is one contradiction inside the acceptance section: four of the eighteen phase-1 scenarios seed bolts, units and work items and assert their transitions, while the proposal says no unit runs and defers seven other scenarios for exactly that reason. Two further scenario claims do not match the scenario files (S22 runs an adapter the proposal does not ship; S13's path is wrong), one sentence claims coverage the suite's `satisfies:` lines do not give, and the Naming paragraph now states the opposite of the documents it cites. The rest is nits.

## The twelve findings

| # | finding | status | reason |
|---|---|---|---|
| 1 | acceptance not enumerated | resolved | lines 324–359 name all thirteen `contract/` files and eighteen scenarios with a path each, and S13/S17/S18 with two host processes; counts match `conformance/` (13 + 44). The paths carry two errors, taken up as new findings 1 and 5 |
| 2 | phase-1 row not covered | resolved | table at lines 255–279 gives one line per section of roadmap row 1; 67 real (171–174), 197 phase 2 (263), 120–122 and 213 phase 3 (272, 277), 95's dictation half phase 4 (268), 206 phase 3 (275) |
| 3 | loaded vs compiled in | resolved | one rule at lines 69–73, and Impact (485–487) agrees: shipped machines in the binary, the organization's types from the blueprints at the shared line; matches model.md §13 (`include_dir`, "type catalogue loader (from the blueprints)"). Citation nit: new finding 6 |
| 4 | leases off the shared line miscite | resolved | lines 64–68 cite `profiles/git-only.yaml` `layout.leases`/`layout.hosts` and model.md §4.2, which say exactly that; 163 is kept for take/renew/expire |
| 5 | adapters unnamed, two read the tracker | resolved | lines 98–102 name the capture box and the chat forward; lines 301–305 keep the pull-request and issue adapters off the git-only profile (C.2) and defer the endpoint to phase 4 (216). S22 undercuts it: new finding 3 |
| 6 | "eight operations" | resolved | lines 45–47: seven operations, eight methods on `StateStore`; §13 lists eight (`list`, `read`, `write_effect`, `lease`, `present`, `receive`, `notify`, `status`) |
| 7 | bootstrap claims 206 | resolved | `organization/bootstrap` is 204, 205, 205a, 207, 207a, 208, 203 (412–414); 206 is phase 3 with 199 kept (275, 311–314) |
| 8 | tool server's second shape | resolved | HTTP only in phase 1 (165–167); stdio and in-process arrive in phase 2 beside the runner (448–450) |
| 9 | organization has five states | resolved, against the review's fix | the proposal's reading is right; see the ruling below |
| 10 | change name vs AGENTS.md and roadmap | not resolved as written | both documents were amended to `the-loop` (AGENTS.md line 40; roadmap.md line 22, Repositories). Proposal lines 495–498 still say both call it `stage1` and need amending. New finding 4 |
| 11 | notify default on a laptop | resolved | lines 88–94: the 30-second `git ls-remote` poll is the default, the git host's call is opt-in because it publishes an address (46, 191, section 9); matches `contract.notify` in the profile |
| 12 | two loose sentences | resolved | line 7–8 "durable in 133's sense — the store is one file on the laptop"; line 3 now reads "numbered to 305 with A.38 beyond them", which is a nit (new finding 8) |

### Ruling on finding 9

The proposal is right. `machines/organization.yaml` region `life`: `connected` → `hosted` on `{ev: org.host_registered, is: true}`, with `register_host {host: first}` as the `always` effect of `connected`; `hosted` is documented "bootstrapped; repositories are proposed and created under it (206), packages are added under it (228)". Clause 204 lists "register the first host" as the last step `flywheel init` drives. So init on one laptop ends in `hosted`, and `hosted` is a bootstrap state, not a tier of A.34. The first review confused the two. One correction to the proposal's own sentence (line 201–202): the loaded definition adds not only `awaiting-app` but `removing` and `removed` (221); see new finding 9.

### A.38 fold-in (306–314)

Consistent with the clauses. 306–311 and 314 are page and chat behaviour phase 1 builds (lines 131–149, 400–408); 312 depends on 281, 283 and 295 (Hobby, the cloud agent's presets, the add-host offers) and 313 on 293 (the remote MCP endpoint), all A.34/A.35, roadmap row 5. The deferral at line 276 is right. The inconsistency is in the roadmap, whose row 1 lists A.38 whole: it should read A.38 (306–311, 314). A blueprints fix, not a proposal fix.

### "State store" and "control plane"

Every use is in the right sense. State store: lines 32–34, 43, 162, 277, 390, 485, 489 (Part B, the `StateStore` trait). Control plane: lines 30, 35–37, 460, 477, all A.37 and always attached to phase 5. Note for blueprints: requirements 92 and S16 still say "stand-in control plane" in the old sense.

## New findings, ranked

### 1. Four phase-1 scenarios run units and work items; the proposal says none do

- **Where:** line 261 ("nothing runs — phase 2"), lines 297–300 ("No bolt, unit, work item, stage or unit type runs"), against the table rows for S05, S13, S17, S18 (lines 341, 344, 346, 347) and the deferral row "S03, S28, S29, S30, S31, X05, X08 | units and work items — phase 2" (line 365).
- **Clause:** 37, 42, 93; `scenarios/S05.yaml`, `S13.yaml`, `S17.yaml`, `S18.yaml`.
- **Why:** S17 seeds `bolt/atlas/plan-rows` and `unit/atlas/status-writer` in `approved` and asserts `unit … from: approved, to: in-flight` with `items_created_once: true`. S18 seeds a work item in `build` with an alive session and asserts `work-item/atlas/wi-418 from: build, to: verify` and a second unit `approved → in-flight`. S13 seeds the same work item and asserts `start_session` and `end_session` on it and `lease/work-item/… held → stale → expired`. S05 seeds a bolt, an in-flight unit and a working work item. Those transitions are the unit and work-item machines running under the `default` type (37). The proposal cannot both claim these four and say no unit runs, and cannot defer S29, S30, S31, X05 and X08 for needing units while keeping S17 and S18, which need the same.
- **Fix:** state one rule. The rule that matches the prototype (README: "git (places and lines are facts in the state store)") and the proposal's own line 291–296 is: every shipped machine ticks in phase 1, unit types included, with the effects of 42 (line, place, merge, landing) bound to the stand-in world as facts; what phase 2 adds is the host binding of those effects and the pane runner. Under that rule rewrite line 261 and 297–300, and re-sort the deferred table: X05 and X08 need no git effect (X05 is a manifest file step and `lease.coverable` evidence; X08 is dictation, pane and place evidence) and belong in phase 1 on both paths; S29 likewise. S30, S31, S14, S32 stay deferred on merges. The other rule, that no unit runs, requires S05, S13, S17 and S18 re-seeded over intents and elaborations, which is a model change proposed to blueprints first (AGENTS.md).

### 2. The suite does not prove 148, 149, 4 or 82 in phase 1

- **Where:** lines 372–376: "The rules those scenarios also carry are proved in phase 1 by the contract set and by the eighteen above: … (148) … (149) … (4) … (82)."
- **Clause:** 148, 149, 4, 82; section 12 ("every conformance scenario states which requirements it satisfies"); `conformance/README.md` (`check.py` fails when a requirement is cited by nothing).
- **Why:** No `satisfies:` line among the eighteen scenarios or the thirteen contract files names 148, 149, 4 or 82. `lease.yaml` cites 128, 134, 150; `present-receive.yaml` cites 1, 6, 129, 137, 153, 154. The only scenarios citing them are the deferred ones: X04 (148), X05 (149, 150), X08 (4, 12, 66, 74), X07 (82). By the suite's own rule the sentence is unsupported.
- **Fix:** bring X05 and X08 into phase 1 (finding 1), and for 148 and 82 say plainly that the page and chat leases and routing by kind are built in phase 1 and proved by X04 and X07 when their presenters and the bell exist, or add the numbers to the `satisfies:` of a phase-1 scenario that actually exercises them, in the model first.

### 3. S22 runs the meeting-transcript adapter, which phase 1 does not ship

- **Where:** line 351 (S22, both paths) against lines 98–102 ("Phase 1 ships two: the page's capture box … and the chat forward … The folder drop, the meeting transcript, the log or monitor webhook follow with their sources").
- **Clause:** 111, 115, 215; `scenarios/S22.yaml`.
- **Why:** S22's `when` is `direct: {adapter: "flywheel capture meeting 2026-09-02-willdan-weekly.vtt"}` twice, a day apart, asserting one capture and zero files written on the second import. That is the meeting adapter's enumerator half (115). "Follow with their sources" names no phase, so the sentence and the table disagree.
- **Fix:** ship the meeting transcript's enumerator in phase 1 as the third adapter — it is arithmetic only, one keyed capture per file, no session (S22 asserts `start_session count: 0`) — and say the judgment half is curation's; or defer S22 with 115 as the reason. The first is the smaller change and keeps 111's idempotent key under test on the git-only path.

### 4. The Naming paragraph is stale

- **Where:** lines 495–498: "AGENTS.md and the roadmap's flywheel-next row still call phase 1 `stage1`; both need that reference amended".
- **Clause:** AGENTS.md line 40 ("`the-loop` is phase 1; then construction, context, dispatch, scale"); roadmap.md line 22 ("`the-loop` is phase 1").
- **Why:** Both already say `the-loop`. The proposal asserts a discrepancy that no longer exists and asks for an amendment already made.
- **Fix:** replace with one sentence: the change is named for the phase as AGENTS.md and the roadmap name it.

### 5. S13's path is wrong

- **Where:** line 344: "S13 … | git-only, two host processes".
- **Clause:** `scenarios/S13.yaml` `profiles: [all]`; `conformance/README.md` ("`all` runs on the stand-in and on every real profile"); `profiles/sessions-stand-in.yaml` `hosts` (`lose: true` "so its heartbeat stops and its leases go stale (S13)").
- **Why:** S13 is an `all` scenario and the stand-in's hosts binding names it. Running it on git-only alone drops the stand-in pass the suite requires.
- **Fix:** "both, two host processes".

### 6. Miscite: machines "inside the binary" are 223 and 224, not 208

- **Where:** line 70: "The shipped machines and profiles are inside the binary and versioned with the set (208, 83)".
- **Clause:** 208 (the blueprints template, the built-repository template, the map schema and derivation table, the shipped skills and deliverables — machines are not in its list); 223 ("A core machine … ships with the release and is never edited by an organization"); 224 (the set version "names the version of every core machine"); model.md §13 (`include_dir`).
- **Why:** 208 names the versioned set but not the machines; the clause that puts core machines in the release is 223, and 224 ties them to the set version. 83 says only that they are data in standalone files.
- **Fix:** "(223, 224, 83; model.md §13)", keeping 208 for the set itself.

### 7. Phase-5 row cites A.37; the roadmap does not

- **Where:** line 30: "| 5 | scale | … and the control plane the binary is invoked by | A.31–A.37 |".
- **Clause:** roadmap row 5 ("A.31–A.36"); roadmap Repositories row (flywheel-cloud is A.37); 297 (the invocation contract "documented in the open-source repository").
- **Why:** The roadmap gives phase 5 A.31–A.36 and places A.37 with flywheel-cloud. The binary's own obligation under A.37, the contract document of 297, has no phase in the roadmap. The proposal's row silently widens phase 5.
- **Fix:** match the roadmap (A.31–A.36) and add one clause: the contract document of 297 is the binary's part of A.37 and is unscheduled in the roadmap.

### 8. "Numbered to 305 with A.38 beyond them"

- **Where:** line 3.
- **Clause:** A.38 is 306–314.
- **Why:** The clauses are numbered to 314; the phrase reads as if A.38 were unnumbered.
- **Fix:** "requirement clauses numbered to 314".

### 9. The loaded organization machine also has `removing` and `removed`

- **Where:** lines 199–203: "whose loaded definition adds `awaiting-app`".
- **Clause:** 221, 4; `machines/organization.yaml` (`hosted` → `removing` on `{response: remove}`, "the operator's dictation `remove <organization>` (4, 221)"; `removed` final).
- **Why:** The machine ticks unchanged in phase 1, so the `remove` dictation is in the tool catalogue (193) from day one, and 221 is A.26 (phase 4). The proposal should say whether phase 1 honours it or reports it as unapplicable (6, 129).
- **Fix:** name all three added states and state the phase-1 handling of `remove`.

## Kept from the first review, still true

Citation discipline holds across the new text: 306–314 (lines 131–149), 150a/151/165 (217–227), the profile's `contract.notify` and `layout.leases` (64–68, 88–94), the stand-in's `hosts` binding (356–359), and `definitions/` byte-identical to `machines/` and `profiles/` (checked file by file). The foreclosure section is unchanged and right.
