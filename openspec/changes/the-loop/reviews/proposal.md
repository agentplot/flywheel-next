# Review: the-loop proposal

**Verdict: revise.** The citations are almost all exact, the real/faked split matches the prototype, the vocabulary is the requirements', and the foreclosure section is the strongest part. What needs to change is the scope statement: the phase-1 acceptance is not enumerated, several sections of the roadmap's phase-1 row are neither claimed nor deferred, and the proposal contradicts itself and the model on whether definitions are loaded or embedded.

## Findings, most serious first

### 1. The phase-1 acceptance is not enumerated

- **Where:** "Scenarios are the acceptance" (lines 160–169); Gate (lines 331–334); `scenarios/conformance` capability (lines 250–253); "Hosts, leases and ownership are specified for one host" (line 137).
- **Clause:** 168, section 12, roadmap "Phase gates"; 134, 162, I15, S13, S17, S18; `models/statechart/conformance/README.md`.
- **Why:** The roadmap ends a phase when "its scenarios pass in conformance", and the proposal never says which. The model's suite is two directories: `contract/` (one scenario per operation of B.1 and per guarantee of B.2, run over the toy `lamp` machine before the domain loads) and `scenarios/` (S1–S34, X1–X9, T1). The proposal names only `scenarios/`, so the suite that actually admits the git-only profile (168, "exercising each operation of B.1 and each guarantee of B.2") is not in the gate. Many of S1–S34 need construction, places or lines that phase 1 keeps as facts in the store, so they cannot run against the git-only profile in phase 1, and the proposal does not say which run on the stand-in only. "Specified for one host" also leaves the two-host git-only scenarios (S13, S17, S18) ambiguous, though they prove the single-writer guarantee that is the heart of C.2. The stand-in profile's `hosts` binding already runs a second host as a process on one laptop, so this costs nothing.
- **Fix:** Add a table naming every `contract/` file and every S/X/T scenario with its phase-1 path (stand-in, git-only in a local bare repository, both, or phase 2 with the reason), and state that S13, S17 and S18 run with two host processes against the local bare repository.

### 2. The roadmap's phase-1 row is not fully covered or deferred

- **Where:** "What becomes real" and "What stays faked"; the capability list.
- **Clause:** roadmap row 1 (A.1–A.16, A.22–A.24, B, C.2); the uncited clauses below.
- **Why:** These clauses of the row are neither claimed nor deferred: A.3 (20–23), A.4 (24–27), A.6 (58–64), A.7 67–74 and 197, A.11 91, A.16 120–122, 95's dictation half (S16), 206, 213. Clause 67 matters most: a session reports its exit "through a command the machinery provides, which writes to the control plane", and `profiles/sessions-stand-in.yaml` plays every scripted exit "through the very same command path" (`flywheel exit|offer|note|refuse`), so the stand-in of 93 cannot be faithful unless that command exists in phase 1. Clause 197 (the tool server refusing a call whose identity is not the pane's) needs panes and belongs to phase 2, but the proposal should say so. 120–122 and 213 need the book and the map (phase 3) and 95's dictation half needs the interpreter (phase 4); each should be deferred with that reason rather than left silent.
- **Fix:** Add one line per section of the row saying real, stand-in or deferred with the phase and reason, and add 67 to the sessions bullet as real (the exit command) with 197 deferred to phase 2.

### 3. "Loaded, not compiled in" contradicts the model and the Impact section

- **Where:** "`definitions/` is loaded, not compiled in" (line 56) against "`flywheel-domain` (the embedded machines ...)" (line 312).
- **Clause:** 57, 83, 85, 228; model.md §13 (`flywheel-domain`: "the machine files embedded with `include_dir`, the type catalogue loader (from the blueprints)").
- **Why:** The prototype's engine loads a directory at runtime (`load_dir` in `crates/flywheel-engine/src/load.rs`); the model embeds the shipped machines at build time and loads the organization's types from the blueprints at the shared line, which is what 57 ("changed by the operator without rebuilding the machinery") and 228 require. The proposal says both and neither reading satisfies 57 on its own. 85 asks only that a new type be no code change, which embedding satisfies for shipped types; it does not settle where an organization's own types come from.
- **Fix:** State one rule: the shipped definitions ship inside the binary and are versioned with the set (208), and an organization's types and packages are read from the blueprints at the shared line (57, 228), so no host is rebuilt for a type.

### 4. Miscite: leases and heartbeats off the shared line

- **Where:** "Leases and heartbeats stay off the shared line so months of renewals add nothing to its history (163, 167)" (lines 53–54).
- **Clause:** 163, 167; `profiles/git-only.yaml` `layout.leases` and `layout.hosts`.
- **Why:** 163 says a lease is taken by a commit that lands and expired by a stated rule; 167 says every write is a commit carrying reason and evidence. Neither says leases live off the shared line. That is the model's answer to section 10's growth question, bound in the profile as `lease/<id>` and `host/<id>` branches holding one orphan commit each. The sentence is right; the citation is not.
- **Fix:** Cite the profile's layout (and model.md §4.2) for the branch-per-lease mechanism and keep 163 for the take/renew/expire rule.

### 5. Adapters: which ship in phase 1, and one of them reads the tracker

- **Where:** `signals/capture-and-curation`: "the shipped adapters' enumerator half" (line 239); "Captures land. Adapters append captures unattended ... (111, 215)" (lines 74–76).
- **Clause:** 215, 216, C.2 preamble, S21.
- **Why:** 215 lists seven adapters. Two of them, the pull-request conversation and the issue tracker, read the git host's issues and reviews, and C.2 says "issues, milestones and boards may exist for people, but the machinery never reads or writes them"; on the git-only profile an adapter reading issues is the machinery reading them. 215's capture endpoint "for callers that cannot reach any host's binary" is one of dispatch's four jobs (216), which is phase 4. The proposal claims the whole set without naming any.
- **Fix:** Name the phase-1 adapters (the page's capture box and the chat forward at least, for 19 and S21), defer the endpoint to phase 4 with 216 as the reason, and say the tracker-reading adapters do not run on the git-only profile.

### 6. "Eight operations" is not the contract's count

- **Where:** "The eight operations — read evidence, write an effect, take/renew/release a lease, present and receive, notify, list, serve the status view (125–132)" (lines 34–35); also lines 223 and 302.
- **Clause:** 125; C.1 and C.2 tables; model.md §13.
- **Why:** 125 enumerates seven operations and both profile tables have seven rows; the sentence itself lists seven and calls them eight. Eight is the method count on the model's `ControlPlane` trait, where present and receive are split. A reader checking the citation finds the count wrong.
- **Fix:** Drop the number, or say "the operations of 125, eight methods on the model's trait because present and receive are two".

### 7. Bootstrap claims 206, which needs the map

- **Where:** `organization/bootstrap` "(204–208, 203)" (lines 241–243).
- **Clause:** 206, 199, 202 (phase 3).
- **Why:** 206 creates a repository from a response "with its map nodes and homes (199, 202)" and runs the first planning's baseline (104). The map is phase 3 and planning phase 2. The proposal's own "not foreclosed" line for 199 (a repository record holds git details alone) is the right phase-1 shape, but the capability claims the whole clause.
- **Fix:** Scope the capability to 204, 205, 205a, 207, 207a and 208, and defer 206 to phase 3 with the map as the reason, keeping adoption by git details alone.

### 8. The tool server's second shape is phase 2 work

- **Where:** "Phase 1 therefore serves the catalogue in both shapes it will need: over HTTP for the page, and over stdio/in-process for sessions (291)" (lines 277–278); `tools/tool-server` "in the model context protocol's shape" (lines 229–231).
- **Clause:** 193, 291, 293; roadmap row 1.
- **Why:** No session runs in phase 1, and the proposal's own foreclosure argument says a later shape is "a new client of an existing server, not a second write path". By that argument deferring stdio forecloses nothing, and 291 and 293 are A.37, which is phase 5. What phase 1 does need on the session side is the exit command of 67 (finding 2), which the stand-in profile binds to `ControlPlane::append`, not to the tool server.
- **Fix:** Keep HTTP as the phase-1 shape, say the catalogue is one object served in more shapes later, and move stdio/in-process to phase 2 beside the runner, or state the reason it must exist in phase 1.

### 9. The organization machine has five states, not four

- **Where:** "`flywheel init` drives the organization machine — absent, blueprints ready, state ready, connected" (lines 123–124).
- **Clause:** 204; `definitions/organization.yaml` (a `hosted` state).
- **Why:** 204 names five states and the loaded definition has `hosted`. Since the definitions are loaded unchanged, the machine phase 1 runs has the fifth state; the proposal should say no phase-1 host reaches it rather than list four.
- **Fix:** Name all five and say `hosted` is entered by no phase-1 host.

### 10. The change's name disagrees with AGENTS.md and the roadmap

- **Where:** the change directory `openspec/changes/the-loop/`.
- **Clause:** AGENTS.md ("`stage1` is phase 1, the loop"); roadmap "Repositories" row for flywheel-next.
- **Why:** Both documents that state how work is done here name the change `stage1`. Either the change is misnamed or both documents are stale; a reader following AGENTS.md will look for a directory that does not exist.
- **Fix:** Rename the change to `stage1`, or amend AGENTS.md and the roadmap in the same commit that lands this proposal.

### 11. Say what notify defaults to on a laptop

- **Where:** "by a call from the git host or a bounded poll, within a stated latency bound (130, 166)" (lines 71–73); "Nothing is published beyond the operator's private network unless the operator says so (46)" (lines 158–159).
- **Clause:** 130, 166, section 9 (the git host "can call a URL when a branch moves, if configured"); `profiles/git-only.yaml` `contract.notify`.
- **Why:** The profile binds the call to a hook on each host reached over a funnel, which publishes a URL beyond the private network. That is fine when the operator chooses it, but the proposal's own rule makes the poll the default and never says so.
- **Fix:** State that on a laptop the bounded poll is the default and the git host's call is the operator's opt-in because it publishes an address.

### 12. Two loose sentences in Why

- **Where:** "305 requirement clauses" (line 3); "Nothing in it is durable" (line 7).
- **Clause:** 133; the prototype's `state/store.json`.
- **Why:** The clauses are numbered to 305 with lettered sub-clauses, so 305 is a number, not a count. The prototype's store is a JSON file on disk that survives a restart of the laptop; it is not durable in 133's sense (loss of every host), which is the sense the proposal means.
- **Fix:** "numbered to 305" and "nothing in it is durable in 133's sense: the store is one file on the laptop".

## What is good and should be kept

- **Citation discipline.** Every claim cites, and apart from findings 4 and 6 every clause checked says what the proposal says it says, including the long C.2 and B.1/B.2 paragraphs and the 150a, 151, 165 host behaviour.
- **The real/faked split.** It matches README.md word for word, `definitions/` is byte-identical to `models/statechart/machines` and `profiles` (only `check.py` and `render.py` are absent), and the engine crate carries no domain name in code.
- **What must not be foreclosed.** The tick as a bounded invocation (75, I14, 136, 231) mapped onto 297's tick mode, same bytes (299), the contract as the only door (125, A.33), sinks per member from the start (236, 236a), and the page as a projection (142, 291, 298) are the right seeds and are stated without pulling phase-5 work forward. Nothing here forecloses the phone requirements: the page works on a phone (155), chat lines link to it (18), links name the organization (205a), and the page's data is already a projection.
- **C.2 used as stated.** No tracker, the git host granted exactly section 9's three things (160, 162, 166), the operator's own commit kept as the response (3, S19), the plan never committed (15) and the status view committed (C.2 table, S20).
- **The tool server as the one write path** (193, 194) with the page, the chat and the machinery as clients, and dictation grammar as itself a tool.
- **The coexistence line** (96) and "Not touched" keep the old flywheel out of scope explicitly.
- **The definitions rule** (model first, `check.py`, copy; no clause invented here) restates AGENTS.md exactly.
