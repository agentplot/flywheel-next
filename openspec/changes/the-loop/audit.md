# Phase 1 audit — the-loop against the model

Model (source of truth): `blueprints/main/design/flywheel-next/` —
`requirements.md`, `surfaces.md`, `models/statechart/{model.md,machines,profiles,conformance}`.
Code: `flywheel-next/main`, audited at HEAD `0c4eadc`.
Scope: A.1–A.16, A.22–A.24, A.38, Part B, C.2; every row of the phase-1
acceptance table in `proposal.md`; every entry of `profiles/git-only.yaml`
and `profiles/host.yaml`.

**Caveat on the working tree.** Another session edited
`crates/flywheel-store-git/src/{store.rs,objects.rs,git.rs}`,
`crates/flywheel-surface/src/page.rs`, `crates/flywheel/tests/curate.rs`
and `crates/flywheel/tests/walkthrough.rs` after `0c4eadc`. `curate.rs`
did not exist at HEAD, so requirements 1, 2, 20, 93b, 109, 110 and 116
below rest partly on uncommitted work. `crates/flywheel-surface/tests/unsigned_in.rs:34`
failed once mid-audit and passed on a re-run; that is churn, not a defect.
Everything else was read at the working tree and is unchanged from HEAD.

## Summary

| classification | requirements | acceptance rows | profile entries |
|---|---|---|---|
| satisfied | 129 | 25 | 37 |
| not satisfied | 25 | 0 | 9 |
| scenario-only | 7 | 9 | 1 |
| out of phase | 43 | 0 | 4 |
| **total** | **204** | **34** | **51** |

Requirements by section:

| section | satisfied | not satisfied | scenario-only | out of phase |
|---|---|---|---|---|
| A.1–A.7 (1–74, 34a) | 39 | 16 | 5 | 15 |
| A.8–A.16 (75–124, 190, 195, 198–202, 211, 215, 317, 318, 93a/b, 199a) | 38 | 1 | 2 | 23 |
| A.22–A.24, A.38, Part B, C.2 | 52 | 8 | 0 | 5 |

`cargo test --workspace` is green: 191 passed, 0 failed, **17 ignored**.
Test-execution time 147 s; wall time 239 s with the build.

---

## Findings, ranked

### 1. The shipped binary performs 20 of the 84 effects and silently counts the rest as done — *not satisfied* (13, 21, 36, 37, 47, 58, 62, 106, 107, 215)

`crates/flywheel/src/host.rs:1160` matches twenty effect names, then
`_ => {}` at `:1349` returning `true` at `:1352` — "an effect no binding
covers is recorded and counts as done". Unbound, and in phase:
`create_items`, `create_bolt`, `record_offers`, `propose_elaboration`,
`run_adapters`, `open_intent`, `declare_services`, `start_service`,
`stop_service`, `route_unit`, `rename_bolt`, `set_type`, `split_intent`,
`record_refusals`, `gather_elaborations`. Each has a body **only** in the
test harness `crates/flywheel-scenario/src/world.rs:86-173`. On the real
binary, approving a unit creates no work items, a session's offer never
becomes a finding record, an adapter never enumerates, and an intent's
change directory is never opened. The proposal's deferral covers only the
effects of 42 (line, place, merge, landing); none of these is one of those.
Compounding it, every bound effect is invoked as `let _ = …`
(`host.rs:1183-1339`), so a bound effect that fails is indistinguishable
from one that succeeded, against 81.
**Fix:** bind the fifteen in `crates/flywheel/src/host.rs`, and make the
fall-through arm return `false` with a run-record problem entry instead of `true`.

### 2. Four evidence atoms the machines gate on are bound in no shipped binding — *not satisfied* (32, 38, 50, 51)

`place.contains_line`, `place.contains_parent`, `place.merge_slot` and
`line.take_due` are answered nowhere outside `crates/flywheel-scenario/src/store.rs`
(`:509`, `:515`, `:530`, `:574`), where they are hardcoded constants.
`crates/flywheel-workspace-recorded/src/lib.rs:85` answers only
`place.exists/absent/merged/conflicted/endpoints*` and `line.exists/absent/landed/landing/acceptance_written`.
`definitions/place.yaml:33` needs `place.exists && place.contains_line` to
leave `preparing`, so a real `flywheel host` can never bring a place to
`ready`: every elaboration and work item stalls, re-running `prepare_place`
each tick. No test drives a real `Host` through the place lifecycle.
Separately the hardcoding drops two ordering guarantees the model states:
`definitions/profiles/record-derived.yaml:85` binds `item.slot_free` to
"host.running < host.bound **and no ready item of an earlier ordinal is
unplaced**" and the code implements only the first conjunct (32); and
`definitions/profiles/host.yaml:399` binds `place.merge_slot` to "no sibling
place of the same line with a lower ordinal is in state merging" against a
constant `true` (38).
**Fix:** answer the four in `flywheel-workspace-recorded` from the store facts
it already writes, and implement the two ordinal conjuncts.

### 3. The acceptance set is not gated: nine of twenty-one scenarios are never asserted — *scenario-only* (acceptance table)

Only `S01` is asserted in the default `cargo test` run
(`crates/flywheel-scenario/tests/conformance_runner.rs:41`). Every other
acceptance-table driver is `#[ignore]`d (`:605`, `:635`, `:700`, `:904`).
**S08, S18, S19, S22, S23, S24 and X01 are referenced by no test at all**;
**S13 and S17** are played through `drive::play` by `real_hosts.rs:190` and
`:239`, which assert trace determinism and `engine_ticks == 0` and never call
`assertions::check`, so their `then:` blocks are dead data. Every driver uses a
hardcoded scenario list; only `the_contract_set_admits_the_profile`
(`conformance_runner.rs:638`) enumerates a directory, and it enumerates
`contract/` only.
`tasks.md:34` (2.15) and `tasks.md:175` (11.4) claim a scenario the acceptance
table lists that every configuration skips is a failure. It is not:
`RunReport::exit_code()` (`conformance/mod.rs:182-194`) returns 0 for a skip,
and `every_listed_scenario_ran()` (`mod.rs:481`) has one caller
(`conformance_runner.rs:516`) which passes a synthetic report and the
one-element list `["S14"]` — never `phase::ACCEPTED`.
**Fix:** enumerate `conformance/scenarios/` in one driver that runs
`phase::ACCEPTED` through `run_one`, and make `exit_code()` non-zero when an
`ACCEPTED` row was skipped.

### 4. `invariants:` in scenario files are never machine-checked — *not satisfied* (I1–I16)

Eighteen scenarios declare `invariants:`. The only uses in the workspace are
`crates/flywheel-scenario/src/conformance/assertions.rs:29` and `:32`, which
interpolate the list into a failure *message string*. No I-number is ever
evaluated. S05's I7, S13's I11, S17's I15 and X08's I1/I6 are decorative.
**Fix:** give each I-number a predicate over the trace and evaluate it in
`assertions::check`, or delete the key from the schema.

### 5. Every link the machinery writes is unreachable — *not satisfied* (308, 205a)

`crates/flywheel-surface/src/links.rs:22-34` writes
`http://<host>/<instance>/<object>` — no port. The router serves `/`,
`/api/tools`, `/api/tools/:name` and `/api/curate` only
(`crates/flywheel-surface/src/http.rs:402-409`), and the host binds
`(name, 4242)` (`crates/flywheel/src/serve.rs:167`). `README.md:68` tells the
operator a third URL, `http://your-laptop.local:4242/`. A chat rendering's link
404s on both path and port, and `crates/flywheel-surface/tests/links.rs:58`
only asserts the link does not name localhost — no test ever fetches one.
205a's settings form and the host's own screen do not exist (`grep -i settings`
over `crates/` is empty).
**Fix:** serve `/<instance>/…` in `http.rs` and build the link from the bound
address including its port, with a test that fetches the link it wrote.

### 6. The tool catalogue is not the one write path — *not satisfied* (193, 311)

`POST /api/curate` (`crates/flywheel-surface/src/http.rs:227-331,407`) is a
page-only write that writes moves into the blueprints and a session report; the
code declares at `:229` that it is "not a tool and not the catalogue's", which
is exactly what 193 forbids. In the other direction `yes all`
(`crates/flywheel-surface/src/chat.rs:519-544`) is a chat-only accelerator the
page deliberately omits (`crates/flywheel-surface/tests/page.rs:551-557`),
against 311's "an accelerator never carries an operation that no control also
carries". `crates/flywheel-surface/tests/catalogue.rs:13` compares page-HTTP
against in-process only; nothing compares the chat's surface.
**Fix:** move `/api/curate` behind a catalogue tool and give the page a
`yes all` control, then extend `catalogue_is_identical_across_callers` to the
chat.

### 7. Requirement 83's diagram checker does not exist in the built repo and is in no gate — *not satisfied* (83)

`definitions/check.py` is a byte-identical copy of the model's file, but its
relative paths resolve only in the model tree. Run from
`flywheel-next/main/definitions` it prints `FAIL requirements not found at
/Users/chuck/Code/github_agentplot/requirements.md` and `requirements: 0 ·
cited: 0`; its diagram pass (`check.py:453`, `glob(ROOT/diagrams/*.svg)`) finds
zero files because `flywheel-next/main/diagrams/` does not exist. Nothing in
`crates/` invokes it and there is no `.github/`. So "a checker ties every
element of a picture to a definition and fails when they disagree" has no
presence in the shipped artifact. (Even in the model tree it globs
`diagrams/*.svg` and skips `diagrams/machines/*.svg`, and validates only shapes
carrying a `data-*` attribute.)
**Fix:** run `check.py` from a `#[test]` with the repository root passed in, and
mirror `diagrams/` beside `definitions/`.

### 8. The cost contract is measured on a code path the host does not take — *scenario-only* (169, git-only `observations`)

`GitStore::begin_tick`/`end_tick` (`crates/flywheel-store-git/src/store.rs:776,783`)
are called only from `crates/flywheel-scenario/src/{runner.rs:282,288,300,326,
statestore.rs:402,416}`. `Host::tick` (`crates/flywheel/src/host.rs:659`) never
calls them. Three consequences:
- `in_tick` is always false on the real host, so `commit_and_push`
  (`store.rs:449`) flushes per effect — one push per effect commit — while the
  runner batches a tick's commits into one push. The two paths spend
  differently, and `conformance/contract/cost.yaml` asserts only the runner's.
- The counters `fetches`, `pushes`, `lease_renewals` and `spawned_at_tick` are
  never reset on the real host, so `cost()` accumulates over its whole life.
- `cost()` computes `read_processes` as `repo.spawned() - pushes` over the
  **state** repository alone (`store.rs:794-802`), so the blueprints reads of
  finding 9 are invisible to `read_processes_per_tick: zero`.
**Fix:** call `begin_tick`/`end_tick` from `Host::tick`, and count the world's
processes into `Cost`.

### 9. Every blueprints read spawns a git process — *not satisfied* (host.yaml `Tools`)

`profiles/host.yaml:15` says "libgit2 through the `gix` crate for reads".
`crates/flywheel-world-host/src/git.rs:126` reads one file as
`git show <line>:<path>` and `:136` lists as `git ls-tree`, both through
`Command::new("git")` at `:40`. `world.rs:180` is the only read path for
captures, signals and moves, so a tick that reads N signals forks N processes —
the same failure the state store fixed in `95e306b`/`0c4eadc`.
**Fix:** open the bare blueprints clone with `gix` and read blobs in process, as
`flywheel-store-git/src/objects.rs` already does.

### 10. A session attempting a line operation is never refused — *not satisfied* (43)

`crates/flywheel/tests/host.rs:468 refusal_reaches_attention` writes
`ThreadEntry { kind: "refusal", operation: "land_line" }` by hand and then
asserts the run record carries it. The only producer of refusal entries in the
product is `crates/flywheel-domain/src/report.rs:155`, reached from
`flywheel refuse` — a session voluntarily reporting. Nothing detects a session
creating a line, merging or landing, and `record_refusals`
(`definitions/session.yaml:63`) is unbound. 43's "a session that attempts one is
refused" has no code.
**Fix:** refuse the line and place effects at the tool server when the caller's
identity is a session, and bind `record_refusals`.

### 11. 191's local router is refused by the code that implements 191 — *not satisfied* (191, 205)

Phase 1 is scoped to the local router (`proposal.md:291`), and
`profiles/host.yaml:310` says "on the operator's own computer the address is a
localhost port". `crates/flywheel-world-host/src/world.rs:61-70` refuses every
localhost address, and `crates/flywheel/src/main.rs:95` defaults
`flywheel init --address` to `http://localhost` — so the default bootstrap
produces a manifest whose host can write no link at all. The port-derived-from-
place half of 45 has no code. Separately, the shared-line checkout lands at
`<root>/<instance>/<repo>` (`world.rs:43-45`) where `profiles/host.yaml:62` says
`<root>/<instance>/<repo>/main`; the code's location is exactly where `places/`
and `bolts/` worktrees must go in phase 2 (`host.yaml:63-64`).
**Fix:** admit `https://<place>.localhost` and a localhost port in
`address_of`, and move the checkout under `/main`.

### 12. An effect repeated inside one tick writes a second commit — *not satisfied* (127)

`write_effect` (`crates/flywheel-store-git/src/store.rs:861-872`) finds a repeat
by scanning history from `self.fetched` and by the `pending` list, which is
appended only on the disconnected path (`:895`). Inside a tick the commit is
local and `self.fetched` does not move (`:449`), so a second `write_effect` with
the same effect id in the same tick passes both checks and commits again.
`records.rs:130` proves the across-tick case only.
**Fix:** check the effect id against `HEAD`, not `self.fetched`, and push every
written id onto `pending` regardless of connectivity.

### 13. 209 and 210 are shape-only — *not satisfied* (209, 210)

`silhouette` (`crates/flywheel-surface/src/page.rs:545-555`) gives `signal` and
`capture` one form and drops `landed`, `instance`, `host`, `sink`, `session`,
`unit` and `rail` into a shared `sessrow`. The committed `status.html`
(`crates/flywheel-domain/src/status.rs:294-320`) renders every kind as an
identical `<article>`, so 209's "no two kinds share one" and "a rendering on any
surface keeps these forms" both fail. The elaboration surface 210 asks for
carries only an id link and an away notice — none of its type, state, pending
decision, document, change-directory records or session activity
(`page.rs:663-740`).
**Fix:** give each kind its own silhouette and render the status view's article
from it; fill the dock's elaboration section from the object's record.

### 14. 19's palette grammar is absent from the page — *not satisfied in part* (19)

The capture half is fully proven: `crates/flywheel-scenario/tests/cascade.rs:197`
asserts the typed text becomes one capture in state `read` with one `ask` signal
in state `unmoved`, `:212` asserts "a page capture needs no reader session", and
`:227-234` asserts no prefix (`intent:`, `chore …`, `bolt …`) opens an intent or
a unit. What is missing is the rest of the clause: "a leading `/` names a
command of 193's catalogue, and a bare number is the reply grammar of 194".
`read_grammar` (`crates/flywheel-surface/src/chat.rs:357`) is called only from
`chat.rs:511`; the page's box captures every keystroke as text
(`page/template.html:643`).
**Model note.** `machines/capture.yaml:47` routes to the no-reader branch only
on `capture.source == forwarded-message`, so by the machine a page capture would
fall through `{always: true}` and charge a capture-reader. The prototype gets the
right outcome by inverting the rule in Rust — `catalogue.rs:512` writes the ask
signal eagerly for every source *except* `forwarded-message`, which makes
`capture.signals_present` true so the machine goes straight to `read`. The
behaviour is correct and tested; it is not in the data, and
`profiles/blueprints.yaml:219` still describes `ensure_signal` as
forwarded-message-only while `:169` says the page's signal comes from it.
**Fix:** add `page-capture` (or an `is_own_excerpt` evidence name) to
`capture.yaml`'s no-reader transition and drop the Rust special case.

### 15. The `asks/<id>.rec` layout entry has no writer — *not satisfied* (git-only `layout.asks`, 116)

`crates/flywheel-store-git/src/layout.rs:38 ask()` has zero callers.
`bootstrap.rs:35` creates `asks/.keep` and nothing ever writes into it.
`apply_move` (`crates/flywheel-domain/src/signals.rs:693`) records a `route`
move's target on the signal but creates no ask object, so 116's "Route records
the chore or the ask that curation offered" has half a mechanism.
**Fix:** write the ask record from the route arm of `apply_move`.

### 16. The no-secret sweep is vacuous where it matters — *not satisfied in part* (204, 207)

`crates/flywheel-world-host/tests/world.rs:238 token_written_nowhere_else`
asserts the key and the minted token appear in no file on any tracked shared
line, and that the manifest holds `key_from` and never the key
(`crates/flywheel/tests/init.rs:82-85`). In that sandbox no `runs/` entry and no
`status.html` exist, so the sweep never covers the run record or the served
page. By construction nothing can reach the page — `page::read`
(`crates/flywheel-surface/src/page.rs:139-198`) takes only the store, the
blueprints `World` and the operator name, and `app_token()`
(`world.rs:152`) has no production caller — but nothing asserts it.
**Fix:** extend the sweep over `runs/**` and the rendered page body after a tick
that heartbeats.

### 17. 145 and S20's two halves never meet — *satisfied, weakly* (132, 145)

`status.html` is genuinely committed by the real host (`host.rs:1054` →
`store.rs:1226`) and the body states the commit and time
(`status.rs:255`, asserted `host.rs:577`). But the no-host read
(`crates/flywheel-store-git/tests/records.rs:358`) commits the literal string
`"<html>as of commit x</html>"`, and S20 (`conformance_runner.rs:610`, ignored)
commits through the scenario store's `write_status`
(`crates/flywheel-scenario/src/store.rs:697`), not through `flywheel host`.
No test reads a body the real host wrote from a clone with nothing running.
**Fix:** in `records.rs:358`, tick a real `Host` first and read its
`status.html` back from a second clone.

### 18. The contract set is behind the group gate against the stated rule

`AGENTS.md:78-81` says `#[ignore]` marks only tests that "start a process of
their own". `the_contract_set_admits_the_profile`
(`conformance_runner.rs:635`) starts no process — it sets no `BINARY_ENV` and
runs in process against a local bare repo — yet it is ignored. The proposal
calls this set "the heart of the gate"; `cargo test --workspace` proves none of
125–137's contract scenarios. `phone_pass_for_every_response_scenario`
(`phone.rs:324`) is worse: it is ignored *and* returns green with an `eprintln!`
when no Chromium is installed (`phone.rs:325-331`), so a browserless CI box
would report 314 satisfied while running nothing.
**Fix:** un-ignore the contract set, and make the phone pass fail rather than
print when no browser is available.

---

## (a) Where the model's reason no longer holds, or a cheaper binding would do

1. **`records.leases` — `git ls-remote` per lease.** `git-only.yaml:31` binds
   `leases` to `git ls-remote origin refs/heads/lease/<id>/lease` and
   `hosts() = refs/heads/host/*/heartbeat`. The code reads the
   remote-tracking refs the tick's fetch already brought
   (`crates/flywheel-store-git/src/store.rs:715-736`,
   `layout.rs:85 HOSTS_PREFIX = refs/remotes/origin/host/`), which is correct,
   cheaper by one process per lease, and is what makes
   `read_processes_per_tick: zero` true. The profile text should say so.
2. **`observations.pushes_per_tick` — "one per effect commit on main".** The
   runner batches a tick's commits into one push
   (`store.rs:445-452`), which satisfies 135, 161 and 167 at a lower cost, and
   the real host does not batch at all (finding 8). The observation's wording
   describes neither path. State the promise as "at most one push per tick plus
   one per lease renewal due" and make both paths take it.
3. **`contract.receive` — the ✅ reaction.** `git-only.yaml:42` makes the
   reaction the operator's proof the push landed. A text reply stands in
   (`chat.rs:586-589`), which satisfies 154 without the platform dependency;
   the reaction is a nicety the wire will carry in group 15.
4. **`layout.object` — `objects/<kind>/<id>/object.rec`.** The code composes
   `objects/{id}/object.rec` with the kind already inside the id
   (`layout.rs:8`). Equivalent, and one fewer thing to keep consistent; the
   layout line should drop `<kind>/`.
5. **`host.yaml Tools` — `gh` is not used anywhere.** True and enforced by
   absence: the only `Command::new` calls in the workspace are two `git`
   (`flywheel-store-git/src/git.rs:65`, `flywheel-world-host/src/git.rs:40`)
   and two binary spawns in the scenario harness.
   `crates/flywheel/tests/boundaries.rs:10-30` enforces that only
   `flywheel-store-git` may link a storage library.

## (b) Tests over 30 seconds

| binary | time | tests |
|---|---|---|
| `crates/flywheel-scenario/tests/cascade.rs` | **60.8 s** | 5 |
| `crates/flywheel/tests/host.rs` | 21.8 s | 18 |

Both spend the time in `git push` subprocesses and nothing else. With
`FLYWHEEL_GIT_TRACE=1` (`crates/flywheel-store-git/src/git.rs:63-82`),
`approving_a_unit_cascades_to_a_merge_and_a_question` (`cascade.rs:56`, 32.3 s)
spawns 221 git processes and spends 20.0 s inside them — **217 pushes at ~92 ms
each**. The chain: `cascade.rs:42-49 run_until_quiet(rt, 60, 3)` and
`rt.settle(20)` drive up to 80 ticks; each tick ends in one
`git push --force-with-lease` (`store.rs:368 push`, `:767 flush`); and
`cascade.rs:12-34 seeded()` builds a fresh bare repo plus clone per test
(`store.rs:1345 sandbox()`), so nothing is shared across the five.
`local_causes_tick_at_once` in `host.rs` (18.1 s) spawns 231 processes, 219 of
them pushes costing 16.5 s, and is the only test in that file building three
repositories. Nothing sleeps and nothing networks: the clock is virtual
(`crates/flywheel-scenario/src/runner.rs:290`), and the only `thread::sleep`
outside ignored tests is 5 ms at `conformance_runner.rs:259`.
`tasks.md:196` records the intent to "build the seeded bare repository once per
test binary and copy it per test"; `rg 'OnceLock|static' crates/flywheel/tests/host.rs`
returns nothing, so it was not done.

## (c) The five shortest paths to testing the loop by hand on a laptop

1. **Bind the fifteen unbound effects in `crates/flywheel/src/host.rs:1160`,
   starting with `create_items`, `record_offers`, `propose_elaboration` and
   `run_adapters`, and make the fall-through arm refuse instead of returning
   `true`.** Without this the binary ticks objects but nothing cascades: a yes on
   a unit produces no work items. This is the single largest gap between what
   the scenarios prove and what `flywheel host` does (finding 1).
2. **Answer `place.contains_line`, `place.contains_parent`, `place.merge_slot`
   and `line.take_due` in `crates/flywheel-workspace-recorded/src/lib.rs:85`.**
   Four evidence names, all derivable from store facts the crate already writes.
   Until they exist a real host cannot move a place past `preparing`, so no
   hand-run reaches a session at all (finding 2).
3. **Serve `/<instance>/…` in `crates/flywheel-surface/src/http.rs:402` and
   build the link from the bound address with its port.** One route and one
   format string turn every rail line, chat rendering and notification into
   something the operator can tap, which is the whole of 308 and what makes a
   hand-run observable from a phone (finding 5).
4. **Add one directory-enumerating driver over `conformance/scenarios/` that
   runs `phase::ACCEPTED` through `run_one`, and un-ignore the contract set.**
   That makes `flywheel scenario run conformance/` the hand-test the README
   promises, and closes the nine unasserted rows in one change (findings 3, 18).
5. **Call `begin_tick`/`end_tick` from `Host::tick`
   (`crates/flywheel/src/host.rs:659`) and seed one bare repository per test
   binary.** Batching a tick's commits into one push takes `cascade.rs` from
   60 s to a few seconds and makes an interactive loop feel live rather than
   stepping at 92 ms per write (findings 8, b).

---

# Appendix A — requirements, per clause

`ign` marks a proof that runs only under `cargo test --workspace -- --include-ignored`.

## A.1 the operator's response

| req | class | code | test | note |
|---|---|---|---|---|
| 1 | satisfied | `flywheel-engine/src/runtime.rs:191`; `flywheel-domain/src/commands.rs:121` | `flywheel-scenario/tests/conformance_runner.rs:41` (S01) | replaying the response leaves `applied_responses` one |
| 2 | satisfied | `flywheel-surface/src/chat.rs:357`; `flywheel-surface/src/page.rs:338` | `flywheel-surface/tests/chat.rs:199`; `flywheel/tests/curate.rs:85` (churn) | the `page_only` flag (`definitions/session.yaml:96`) is read by no code |
| 3 | not satisfied | `flywheel-store-git/src/store.rs:1301`, called only from `flywheel-scenario/src/runner.rs:494` | `flywheel-store-git/tests/records.rs:331` | the read half is proven; the host loop never turns the operator's own commit into a response, and S19 is run by nothing |
| 4 | not satisfied | `flywheel-domain/src/commands.rs:174 DICTATION_TOOLS` | `flywheel-surface/tests/bodies.rs:121`; X08 (ign) | "send back to a stage" is in neither the catalogue nor `DICTATION_TOOLS`, though `definitions/stage.yaml:48` expects it |
| 5 | satisfied | `definitions/unit.yaml:95` | `flywheel-scenario/tests/cascade.rs:56`; S04 (ign) | no items before approval |
| 6 | satisfied | `flywheel-domain/src/derived.rs:207`; `definitions/engine/response.yaml:40` | `conformance_runner.rs:41`; `cascade.rs:246` | the "unapplicable is reported" half has only a negative assertion |

## A.2 the rail

| req | class | code | test | note |
|---|---|---|---|---|
| 7 | satisfied | `flywheel-engine/src/rail.rs:23` | `flywheel-surface/tests/page.rs:60`; S05 (ign) | derived per request, nothing stored |
| 8 | satisfied | `flywheel-engine/src/rail.rs` | `page.rs:60` | no batching |
| 9 | satisfied | `flywheel-engine/src/runtime.rs:191` | `flywheel-engine/tests/lamp.rs:385` | creation and retraction both asserted |
| 10 | satisfied | `definitions/{intent,bolt,unit,elaboration,session}.yaml` | `cascade.rs:56`, `:246`; `curate.rs:85`; S02/S04/S07 (ign) | six of seven kinds driven; the chore kind is phase 2; no test enumerates the catalogue |
| 11 | satisfied | `flywheel-engine/src/rail.rs:36,78` | `lamp.rs:474`; `chat.rs:288`, `:354` | |
| 12 | satisfied | `commands.rs:174`; `flywheel-surface/src/catalogue.rs:252` | `flywheel-surface/tests/dictation.rs:146`; `cascade.rs:108` | |
| 13 | satisfied | `definitions/unit.yaml:95` | `cascade.rs:56` | on the scenario harness only — see finding 1 for the binary |
| 14 | satisfied | `flywheel-domain/src/sinks.rs` | `lamp.rs:601`; `chat.rs:571`, `:528` | |
| 15 | satisfied | `flywheel-engine/src/runtime.rs` Register | `lamp.rs:441`, `:346`; `page.rs:60` | |
| 16 | not satisfied | `definitions/unit.yaml:71`; bodies only in `flywheel-scenario/src/world.rs:86,95` | none | `route_unit`/`rename_bolt` unbound on the host, so a rename is recorded and counted as done |
| 17 | out of phase | — | — | `proposal.md`: the unit proposal document waits on the runner |
| 18 | satisfied | `chat.rs:475` | `chat.rs:155`, `:449` | |
| 19 | not satisfied | `page/template.html:643`; `catalogue.rs:512` | `cascade.rs:197-234`; `page.rs:290`, `:348` | capture/one-ask-signal/no-reader/no-parsing all proven; the `/` command and bare-number arms of the palette are absent — finding 14 |

## A.3–A.4 intents, curation, elaborations

| req | class | code | test | note |
|---|---|---|---|---|
| 20 | satisfied | `flywheel-domain/src/signals.rs:878`; `flywheel/src/host.rs:1311` | `flywheel/tests/curate.rs:85` (churn) | end-to-end on the real host |
| 21 | not satisfied | `definitions/intent.yaml:42 propose_elaboration` unbound; `flywheel-scenario/src/store.rs:578` pins `intent.material_pending` false | `lamp.rs:528` (toy machine) | the intent's own one-proposal path can never fire |
| 22 | satisfied | `definitions/intent.yaml:44-62` | S07 (ign) | |
| 23 | out of phase | — | — | the design book is phase 3 |
| 24 | satisfied | `definitions/elaboration.yaml` | `conformance_runner.rs:41` (S01) | |
| 25 | satisfied | `definitions/elaboration-types/*` | S02 (ign); `flywheel-sessions-operator/tests/operator.rs:96` | X01 is run by no test |
| 26 | satisfied | `definitions/elaboration-types/standing@2.yaml:42` | S02 (ign) | |
| 27 | not satisfied | `definitions/elaboration.yaml:39 set_type`; body only in `world.rs:103` | `lamp.rs:570` (toy) | `set_type` is bound in no shipped binding |

## A.5 planning and construction

| req | class | code | test | note |
|---|---|---|---|---|
| 28, 29 | out of phase | — | — | planning waits on the runner |
| 30 | satisfied | `definitions/bolt.yaml` | `cascade.rs:246` | two bolts proceed independently |
| 31 | satisfied | `definitions/work-item.yaml:26`; `unit.yaml:118` | S29 (ign) | |
| 32 | not satisfied | `flywheel-scenario/src/store.rs:574` vs `definitions/profiles/record-derived.yaml:85` | S29 (ign) | the ordinal conjunct of `item.slot_free` is dropped — finding 2 |
| 33 | scenario-only | `definitions/bolt.yaml:1` comment | none | S28 deferred; no ageing-out code and nothing asserting its absence |
| 34 | scenario-only | `definitions/unit.yaml:62` | none | S28 and X10 both deferred |
| 34a | out of phase | `definitions/unit.yaml:25` | — | claims are A.14, phase 3 |
| 35, 36 | out of phase | — | — | claim-moved is phase 3; the proposal document waits on the runner |
| 37 | satisfied | `definitions/work-item.yaml:41`; `unit-types/default@5.yaml` | `cascade.rs:56` | the type is the machine driving the item |
| 38 | not satisfied | `store.rs:515 place.merge_slot => true` vs `profiles/host.yaml:399` | S14 skipped | "one at a time in a fixed order" unenforced — finding 2 |
| 39 | satisfied | `definitions/bolt.yaml:48` | `cascade.rs:246` | |
| 40 | scenario-only | `definitions/bolt.yaml:83` | none | S33 skipped; `flywheel-workspace-recorded/src/lib.rs:142 land_line` always succeeds |
| 41 | scenario-only | `definitions/stage.yaml:52`; `work-item.yaml:72` | none | S14 skipped; no script produces a not-done verdict |
| 42 | satisfied | `flywheel-workspace-recorded/src/lib.rs:113-190` | `flywheel-workspace-recorded/tests/recorded.rs:23` | in phase as the 93a stand-in |
| 43 | not satisfied | `flywheel-domain/src/report.rs:155`; `record_refusals` unbound | `flywheel/tests/host.rs:468` | the test writes the refusal by hand — finding 10 |
| 44 | satisfied | `definitions/bolt.yaml:113` | `cascade.rs:147` | "refreshed after each merge" unexercised (`place.contains_line` hardcoded) |
| 45 | scenario-only | `definitions/service.yaml`; `world.rs:120` | none | X09 skipped; the port comes from the declaration, not from the place |
| 46 | not satisfied | `definitions/place.yaml:46 record_endpoints` | `cascade.rs:147` (record only) | "shown on the page beside the bolt" exists nowhere |
| 47 | not satisfied | `definitions/service.yaml`; `world.rs:108` | `cascade.rs:147`; `dictation.rs:189` | the machine and the dictations work; the page shows no service |
| 48 | not satisfied | no `service` arm in `flywheel/src/main.rs:25` | none | the command a session would use does not exist |
| 49 | satisfied | `definitions/line.yaml`; `intent.yaml:75`; `bolt.yaml:107` | S07 (ign) | the built side (S34, X03) is skipped |
| 50 | not satisfied | `store.rs:530 line.take_due => false`; unbound in `flywheel-workspace-recorded` | S33/X03 skipped | the take transition can never fire — finding 2 |
| 51 | not satisfied | `store.rs:509 place.contains_line => true` | S32 skipped | `place.behind` unreachable; the rebase and `tell_moved` never exercised |
| 52 | not satisfied | `flywheel-workspace-recorded/src/lib.rs:126` always returns `TakeOutcome::Done` | S32/X03 skipped | no conflict can arise |
| 53 | out of phase | `store.rs:534` pins `line.policy` to `direct` | — | pull-request landing waits on the runner |
| 54 | satisfied | `definitions/elaboration.yaml:58`; `intent.yaml:64` | S07 (ign) | the self-closing place-merge half (S34) is skipped |
| 55 | satisfied | `definitions/place.yaml:96`; `flywheel-workspace-recorded/src/lib.rs:181` | `cascade.rs:108` | the reconciliation sweep and the operator's hold are unimplemented |
| 56 | not satisfied | `store.rs:563 stage.agents => ["agent"]` vs `definitions/stage.yaml:33` | S26 skipped | one session per stage always; the join rule never exercised |
| 57 | satisfied | `flywheel-domain/src/blueprints.rs` | `flywheel-domain/tests/blueprints.rs:13`, `:40` | a type added in the blueprints runs with no code change |

## A.6–A.7 findings, chores, sessions

| req | class | code | test | note |
|---|---|---|---|---|
| 58 | satisfied | `flywheel-domain/src/offers.rs:111`; `definitions/session.yaml:60` | S04 (ign) | `record_offers` is bound only in the scenario world — finding 1 |
| 59, 60, 61, 63, 64 | out of phase | — | — | chores as units of the chore type are phase 2 |
| 62 | satisfied | `offers.rs:2,111`; `flywheel-domain/src/report.rs` | `flywheel/tests/report.rs:33`; S04 | the record never holds the text |
| 65 | satisfied | `report.rs`; `flywheel/src/main.rs:154-192` | `report.rs:33`, `:119` | all five exits, anything else refused |
| 66 | satisfied | `report.rs` | `report.rs:33`; X08 (ign) | the command writes nowhere but the thread |
| 67 | satisfied | `flywheel/src/main.rs:508` | `flywheel-scenario/tests/scripted_sessions.rs:103` | |
| 68, 70 | out of phase | — | — | need the pane runner |
| 69 | satisfied | `catalogue.rs:399`; `definitions/operator-session.yaml` | `flywheel-surface/tests/bodies.rs:43` | weak: X01 is run by nothing; nothing asserts the session is on no thread |
| 71 | satisfied | `definitions/session.yaml:57` | S04 (ign) | `session_interrupted: false` |
| 72 | satisfied | `definitions/session.yaml:44`; `flywheel-sessions-operator/src/lib.rs:200` | S06 (ign); `operator.rs:32` | success judged by `session.pane`, not by the command's return |
| 73 | satisfied | `flywheel-engine/src/tick.rs` | `lamp.rs:212`; `records.rs:130`; S06 | |
| 74 | satisfied | `definitions/work-item.yaml:44`; `unit.yaml:139` | `cascade.rs:108`; X08 (ign) | |
| 197 | out of phase | — | — | the tool server refusing a call whose identity is not the pane's needs panes |

## A.8–A.13 state, observability, engine, instructions, scenarios, coexistence

| req | class | code | test | note |
|---|---|---|---|---|
| 75 | satisfied | `flywheel-store-git/src/store.rs` | `flywheel-store-git/tests/disconnected.rs:232` | the whole-rail restart (S05) is behind the group gate |
| 76, 77 | satisfied | `flywheel/src/host.rs:1014-1046`; `store.rs:1230` | `flywheel/tests/host.rs:632` | drift rewritten from source and reported with both values |
| 78 | satisfied | `flywheel-world-host/src/world.rs:197` | `records.rs:113`; `flywheel/tests/init.rs:91` | |
| 79 | satisfied | `host.rs:349 RunEntry` | `host.rs:357` | reason and the evidence the guard read |
| 80 | satisfied | `host.rs:955-978` | `host.rs:388` | expected beside delivered |
| 81 | satisfied | `host.rs report_problem` | `host.rs:443` | no work is filed; weakened by `let _ =` on every effect (finding 1) |
| 82 | satisfied | `flywheel-domain/src/sinks.rs:50,115` | `chat.rs:626` | the `bell` sink is a doc comment (X07 deferred); "a construction host is silent by default" has no code |
| 83 | not satisfied | `definitions/check.py:453` | none | finding 7 |
| 84 | satisfied | `flywheel-scenario/src/conformance/drive.rs` | `conformance_runner.rs:41` | described state in, decisions asserted, no service |
| 85 | satisfied | `flywheel-domain/src/blueprints.rs` | `blueprints.rs:13` | no decision kind is a literal in non-test Rust |
| 86 | satisfied | `flywheel-engine/src/**` (zero domain-word hits) | `flywheel-engine/tests/domain_boundary.rs:115` | the checker's `OBJECT_NAMES` holds `"work item"` with a space, so `work_item` would pass |
| 87 | satisfied | `flywheel-atoms/src/atoms.rs` | `flywheel-atoms/tests/registry.rs:13` | registry is exactly the atoms file, both directions |
| 88, 89, 90 | satisfied | `flywheel-domain/src/instructions.rs:143`; `order.rs:2`; `flywheel/src/render_order.rs:21` | `flywheel/tests/render_order.rs:19` | four sections and no fifth; nothing started |
| 91 | satisfied in part | `instructions.rs:6` | `render_order.rs:97` | the "a change is a chore" half is phase 2 |
| 92 | satisfied | `flywheel-scenario/src/conformance/mod.rs:27-33` | `conformance_runner.rs:203` | git-only is the only profile; the stand-in store is retired |
| 93 | satisfied | `flywheel-scenario/src/sessions.rs:1` | `conformance_runner.rs:41` | the proof the scripted exit goes through the real command is ign |
| 93a | satisfied | `flywheel-workspace-recorded/src/lib.rs:1`; `manifest.rs:37` | `host.rs:311`; `recorded.rs:23`; `conformance_runner.rs:464` | real-workspace scenarios are skipped, never run against it |
| 93b | satisfied | `flywheel-sessions-operator/src/lib.rs`; `host.rs:1216` | `curate.rs:85` (churn); `operator.rs:32` | the page's submit is the same exit record |
| 94 | satisfied | `flywheel-atoms/src/conformance.rs:5`; `conformance/schema.json` | `conformance_runner.rs:63` | every `then` key the schema allows is evaluated, and vice versa |
| 95 | out of phase | — | trace half: `conformance_runner.rs:129`, `:167` | dictation-into-data needs the interpreter, phase 4 |
| 96 | satisfied | `flywheel/src/init.rs:253` | `init.rs:169` | stands two new flywheels beside each other, not one beside the current |

## A.14 claims, as-built, the ledger — phase 3

| req | class | note |
|---|---|---|
| 97, 99–105, 195 | out of phase | no claim, ledger or verdict code in `crates/` |
| 98 | out of phase | phase-1 residue satisfied: `flywheel-world-host/src/manifest.rs:19` holds git details alone, `flywheel/tests/init.rs:260`. `Repository` has no `deny_unknown_fields`, so a hand-declared kind would be ignored rather than refused |

## A.15 signals and curation

| req | class | code | test | note |
|---|---|---|---|---|
| 106 | satisfied in part | `flywheel-domain/src/adapters.rs`; `signals.rs` | `flywheel/tests/signals.rs:46`, `:236` | "never reads a signal except through curation" unproven; `run_adapters` is unbound (finding 1) |
| 107 | satisfied | `signals.rs:812`, `:621` | `signals.rs:412` | the "splitting a cluster" half has no tool and `split_intent` is unbound |
| 108 | scenario-only | none | S08, run by no test | the claims it clusters against are phase 3 |
| 109 | satisfied | `signals.rs:874` | `page.rs:406`; `curate.rs:85` (churn) | count, sources, span by event date |
| 110 | satisfied | `flywheel-domain/src/cadence.rs` | `signals.rs:628`; `curate.rs:85` (churn) | includes a person writing the records by hand |
| 111 | satisfied | `signals.rs write_capture` | `signals.rs:46 capture_keyed_once`; `chat.rs:845` | the same source event twice yields one capture, asserted directly; S22 is run by nothing |
| 112 | satisfied | `chat.rs:418`; `catalogue.rs capture`; `page/template.html:374` | `chat.rs:781`; `page.rs:348` | the folder drop is not shipped |
| 113, 114 | satisfied | `signals.rs:272`; `SIGNAL_FORMAT` | `signals.rs:236`, `:311` | older signals read unconverted, reading writes nothing back |
| 115 | satisfied | `adapters.rs:1` | `chat.rs:781`; `page.rs:348` | enumerator writes zero signals |
| 116 | satisfied | `signals.rs:664` | `signals.rs:528`; `curate.rs:85` (churn) | route records no ask object — finding 15; challenge's verdict-staling is phase 3 |
| 117 | scenario-only | `signals.rs:980`; `host.rs:1320` | none — only S23, which no test runs | code exists, nothing exercises it |
| 118 | satisfied | `flywheel-domain/src/status.rs:41` | `signals.rs:735`; `page.rs:406` | |
| 215 | satisfied in part | `adapters.rs:47`; `chat.rs:418`; `catalogue.rs` | `chat.rs:781`; `page.rs:348` | the meeting adapter has no test at all (S22 runs nowhere) and `run_adapters` is unbound |

## A.16 default instructions and the review surface

| req | class | code | test | note |
|---|---|---|---|---|
| 119 | satisfied | `instructions.rs:8`; `deliverables.rs:8` | `domain_boundary.rs:139` | bans instruction text and `include_str!` in the engine |
| 120, 121, 122 | out of phase | — | — | phase 3 |
| 123, 124 | satisfied | `instructions/set.yaml`; `flywheel/src/render_order.rs:21` | `render_order.rs:19`, `:97` | a version the set lacks is refused |
| 190 | satisfied | `flywheel-domain/src/deliverables.rs` | `render_order.rs:19` | |
| 198–202, 211, 317, 318 | out of phase | `bootstrap.rs:24` writes empty map files | — | phase 3; the string `199a` appears nowhere in `crates/`, `definitions/` or `openspec/` |

## A.22–A.24 endpoints, files, bootstrapping

| req | class | code | test | note |
|---|---|---|---|---|
| 191 | not satisfied | `flywheel-world-host/src/world.rs:13,61-70` | `flywheel-world-host/tests/world.rs:359` | the local router — the one phase 1 scoped — is refused; finding 11 |
| 203 | satisfied | `flywheel-world-host/src/prefix.rs:25`; `world.rs:198-225` | `flywheel/tests/init.rs:138` | prefix rule, the response exception and the state repo's exemption |
| 204 | satisfied | `flywheel-world-host/src/bootstrap.rs:204` | `init.rs:62`, `:91`, `:114` | idempotent and resumable; every singleton (`init.rs:126`, `curation`) made once |
| 205 | not satisfied | `world.rs:43-45` | `world.rs:187`, `:211` | the checkout is at `<repo>` where `profiles/host.yaml:62` says `<repo>/main`; finding 11 |
| 205a | not satisfied | `flywheel-surface/src/http.rs:402-409` | `world.rs:359` (the address is written only) | no host serves `/<instance>/…`; the settings form does not exist |
| 206 | out of phase | `bootstrap.rs:82` | — | map nodes, homes and the first planning baseline are phase 3 |
| 207 | satisfied | `manifest.rs:87-100`; `world.rs:152-171` | `world.rs:238 token_written_nowhere_else`; `:281` | the sweep is vacuous for `runs/**` and the page — finding 16 |
| 207a | satisfied | `bootstrap.rs:136-160` | `world.rs:238`; `init.rs:62` | no agent places the key; the refusal names 207a |
| 208 | satisfied | `bootstrap.rs:121,129` | `world.rs:325`; `init.rs:283` | a newer set upgrades nothing on its own |

## A.38 the phone

| req | class | code | test | note |
|---|---|---|---|---|
| 306 | satisfied | `page/template.html` | `page.rs:145` | one bundle, both widths |
| 307 | satisfied | `page.rs:31 VERSION` | `page.rs:145`, `:187` | one document, version = the binary's |
| 308 | not satisfied | `links.rs:22-34`; `http.rs:402-409`; `serve.rs:167` | `links.rs:58` | finding 5 |
| 309 | satisfied | `chat.rs:30-77` | `chat.rs:449` | the page pushes nothing |
| 310 | satisfied | `page.rs:236-260` | `page.rs:60`, `:209`; `phone.rs:289` (ign) | one request, no client state, nothing fetched |
| 311 | not satisfied | `page/template.html:546,583` | `phone.rs:53` (ign) | `yes all` is a chat-only accelerator with no control — finding 6 |
| 312, 313 | out of phase | — | — | phase 5 |
| 314 | satisfied | `flywheel-scenario/src/conformance phone_set` | `phone.rs:324` (ign, self-skips without a browser); `phone.rs:152` | covers only S01, S04, S07, S13, S23; truncates at the response and never runs the scenario's `then:`; the mockups-at-390px half has no test |

## Part B — the state store

| req | class | code | test | note |
|---|---|---|---|---|
| 125 | satisfied | `flywheel-atoms/src/traits.rs:223-269` | `flywheel/tests/boundaries.rs`; `contract/binding.yaml` (ign) | `Records` carries eight operations where `model.md:602` names six; `thread` and `hosts` are undeclared as a contract change |
| 126 | satisfied | `store.rs:806` | `records.rs:113` | |
| 127 | satisfied | `store.rs:861-904` | `records.rs:130` | across ticks only — finding 12 |
| 128 | satisfied | `store.rs:906-1019` | `records.rs:170` | stale 5 m, expiry 24 h, renewal 1 min (`store.rs:34`) |
| 129 | satisfied | `store.rs:1046-1067` | `records.rs:219`; `contract/present-receive.yaml` (ign) | the plannotator half has no code and no stated deferral |
| 130 | satisfied | `store.rs:1021-1034`, `:1134` | `records.rs:248` | a never-notified host converges |
| 131 | satisfied | `store.rs:834` | `records.rs:77` | |
| 132 | satisfied | `store.rs:841`, `:1226`; `host.rs:1014-1055` | `records.rs:281`; `host.rs:631` | finding 17 |
| 133 | satisfied | `world.rs:129-132` | `records.rs:61` | read back by a host that never saw the write |
| 134 | satisfied | `store.rs:619-651` | `records.rs:301` | the loser discards and re-reads |
| 135 | satisfied | `store.rs:403-460` | `contract/atomic.yaml` (ign) | only proof is behind the group gate |
| 136 | satisfied | `flywheel-domain/src/status.rs:120-212` | `records.rs:281`; `contract/derivable.yaml` (ign) | a fresh clone renders the identical body |
| 137 | satisfied | `flywheel-store-git/src/layout.rs:61` | `records.rs:219`; `layout.rs:180` | |
| 138, 139, 140 | satisfied | `flywheel-domain/src/profile.rs` | `flywheel-domain/tests/binding.rs:10`, `:21`, `:79` | an incomplete binding names the missing evidence name |
| 141 | satisfied | `status.rs:16-46` | `host.rs:525` | four groups, holder, runner, liveness |
| 142 | satisfied | `host.rs:1042-1054` | `host.rs:631` | |
| 143 | satisfied | `status.rs:249`; the page board | `host.rs:525`; `page.rs:127` | its scenario S12 is deferred |
| 144 | satisfied | `status.rs:31` | `host.rs:584` | its scenario S30 is deferred |
| 145 | satisfied | `status.rs:255`; `store.rs:1226` | `host.rs:575`; `records.rs:281`; S20 (ign) | finding 17 |
| 146 | satisfied | `status.rs:120-212` | `host.rs:525` | 146 permits the first build not to serve the per-repository views |
| 147 | satisfied | `host.rs:430` | `host.rs:702` | two hosts, one state repository |
| 148 | satisfied | `sinks.rs:430,462` | `chat.rs:65`, `:121` | X04 is deferred; the code tests carry it |
| 149 | satisfied | `host.rs:535-545` | `host.rs:157`; X05 (ign) | |
| 150 | satisfied | `host.rs:851`, `:883` | `host.rs:702` | |
| 150a | satisfied | `host.rs:479-531` | `host.rs:215`, `:269` | |
| 151 | satisfied | `store.rs:921-923,974-977`; `host.rs:556-559` | `disconnected.rs:114`, `:182` | S18 is run by no test |
| 152 | satisfied | `catalogue.rs`; `chat.rs:557` | `chat.rs:155` | |
| 153 | satisfied | `store.rs:1046-1063` | `flywheel/tests/served.rs:71` | `given_by` read back from a second clone |
| 154 | satisfied | `chat.rs:586-589`; `page.rs:405` | `chat.rs:155`; `served.rs:142` | the profile's ✅ reaction is not implemented |
| 155 | satisfied | `http.rs:108-129` | `flywheel-surface/tests/unsigned_in.rs:63` | |
| 193 | not satisfied | `http.rs:227-331,407` | `flywheel-surface/tests/catalogue.rs:13` | `POST /api/curate` is a page-only write outside the catalogue — finding 6 |
| 193a | satisfied | `catalogue.rs:158`; `http.rs:142` | `catalogue.rs:13` | the permission filter is 293, phase 5 |
| 194 | satisfied | `chat.rs:352-414` | `chat.rs:247`, `:396`; `page.rs:288`; `cascade.rs:227` | closed word list, free text neither parsed nor guessed; the interpreter half is phase 4 |
| 209 | not satisfied | `page.rs:545-555`; `status.rs:294-320` | `page.rs:236` | finding 13 |
| 210 | not satisfied | `page.rs:663-740` | `page.rs:236` | finding 13 |
| 213 | out of phase | — | — | needs the book and the map, phase 3 |
| 214 | out of phase | — | `page.rs:535 NOT_THIS_PHASE` | phase 2 |

## C.2 the git-only profile

| req | class | code | test | note |
|---|---|---|---|---|
| 160 | satisfied | `flywheel-store-git/src/lib.rs:1-22`; `layout.rs` | `records.rs:61` | |
| 161 | satisfied | `store.rs:420-512`; `Landed::Pending` | `records.rs:61`; `disconnected.rs:232` | |
| 162 | satisfied | `flywheel-store-git/src/git.rs:148-161` | `records.rs:301`; S17 (ign, `then:` unasserted) | |
| 163 | satisfied | `store.rs:967-991`; `RENEW_EVERY` `:34` | `records.rs:170`; `disconnected.rs:60` | leases on orphan branches, never files on `main` |
| 164 | satisfied | `store.rs:1046`, `:1289`, `OPERATOR:24` | `records.rs:331`; `served.rs:71` | one named writer; `commit/<sha>` |
| 165 | satisfied | `store.rs:147-186` | `host.rs:93 tick_fetches_first`; `disconnected.rs:182` | fetch is the first operation of every tick |
| 166 | satisfied | `store.rs:1021`, `:1134` (30 s) | `records.rs:248`; `disconnected.rs:267` | the webhook half is absent — see the profile table below |
| 167 | satisfied | `store.rs:886-889` | `records.rs:130` | id, reason, evidence, object, effect in the message |

---

# Appendix B — the acceptance table

| row | class | driven by | note |
|---|---|---|---|
| contract/read | satisfied | `conformance_runner.rs:635` (ign) | `:738` also runs it in the default gate but asserts only `exit_code != 3`, so a `then:` failure passes there |
| contract/write-effect, lease, present-receive, notify, list, status | satisfied | `conformance_runner.rs:635` (ign); status also `:609` | asserts `Status::Passed` |
| contract/durable, single-writer, atomic, derivable, response-once, binding | satisfied | `conformance_runner.rs:635` (ign) | |
| S01 | satisfied | `conformance_runner.rs:41` **default gate** | the only fully gated row |
| S02, S04, S05, S06, S07 | satisfied | `conformance_runner.rs:709-713` (ign) | |
| S16, S21, X08 | satisfied | `conformance_runner.rs:911-913` (ign) | |
| S20, S29, X05 | satisfied | `conformance_runner.rs:607-610` (ign) | |
| **S08** | scenario-only | nothing | no test references it |
| **S13** | scenario-only | `real_hosts.rs:190` (ign) via `drive::play` | asserts trace determinism only; its `transitions`, `decisions`, `status`, `effects`, `leases` and `state_store` are never checked |
| **S17** | scenario-only | `real_hosts.rs:239` (ign) via `drive::play` | asserts `engine_ticks == 0` and a non-empty store only |
| **S18, S19, S22, S23, S24, X01** | scenario-only | nothing | their `then.state_store` keys (`signal_moves_stored: 20`, `capture_files_written_on_second_import: 0`, `response_derived_from_fetch: true`, `dictation_bypassed_plan: true`) are dead data |

Mirror check: `diff -r` between the model's `conformance/` and the code's is
clean (65 files, zero differences), and the model's `machines/` matches the
code's `definitions/` with `profiles/` nested one level deeper. Nothing in the
test suite compares the two: `run_record_hash_matches_definitions`
(`conformance_runner.rs:738`) compares on-disk `definitions/` against the set
embedded in the binary, not against the model.

Every `then:` key the schema allows has handling in `assertions.rs`, and
`expect_keys_are_exhaustive` (`conformance_runner.rs:63`, not ignored) enforces
parity both ways. Two gaps sit one level down: `invariants:` is never evaluated
(finding 4), and `effects` is an open set unless `effects_closed: true`, which
only four files set (`contract/read.yaml`, S02, S05, S11) — for the other 41
an unlisted extra effect is tolerated in silence.

---

# Appendix C — the profile entries

## `profiles/git-only.yaml` — Tools

| entry | class | evidence |
|---|---|---|
| `gix` for reads | satisfied | `flywheel-store-git/src/objects.rs`; `records.rs:113` |
| `gix` for the fetch | satisfied | `store.rs:147-165`; the git trace shows zero fetch processes |
| `gix` for object writes, in the host's process | satisfied | `store.rs:404-416 commit_worktree` |
| the `git` binary for the guarded push alone | satisfied | `git.rs:65` is the crate's only `Command::new`; `git.rs:103,124-134` also run `init --bare`, `checkout` and `rev-parse`, all outside the tick |
| recutils files parsed by the binary | satisfied | `flywheel-engine/src/rec.rs`; no recutils process anywhere |
| the chat, page, bell and review surfaces run by the host that presents each sink | satisfied | `sinks.rs:430`; `chat.rs:65`. Bell is phase 2 (X07) and the plannotator review surface is out of phase with 17 |
| the pages committed as files | satisfied | `store.rs:1226 commit_status`; `records.rs:281` |

## `profiles/git-only.yaml` — layout

| entry | class | evidence |
|---|---|---|
| `repository` | satisfied | `bootstrap.rs`; `world.rs:129` |
| `object` | satisfied | `layout.rs:8` — composes `objects/{id}/object.rec` with the kind inside the id; see (a)(4) |
| `thread` | satisfied | `layout.rs:14`; `records.rs:77` |
| `responses` | satisfied | `layout.rs:31`; `records.rs:219` |
| `asks` | **not satisfied** | `layout.rs:38 ask()` has zero callers — finding 15 |
| `pages` | satisfied | `store.rs:1226`; finding 17 |
| `run_record` | satisfied | `layout.rs:44`; written from `host.rs:782`; no test pins the literal path |
| `leases` | satisfied | `layout.rs:59,69`; `layout.rs:134 a_lease_ref_is_never_a_prefix_of_another`; `disconnected.rs:60` |
| `hosts` | satisfied | `layout.rs:76,143`; the dispatcher heartbeat is phase 4 |

## `profiles/git-only.yaml` — records, contract, guarantees

| entry | class | evidence |
|---|---|---|
| `records.get` | satisfied | `store.rs:806`; `records.rs:113` |
| `records.put` | satisfied | `store.rs:619-651`; three rejections then report; `records.rs:301` |
| `records.append` | satisfied | `records.rs:77` |
| `records.list` | satisfied | in-process gix tree walk plus `changed_paths`, cheaper than the stated `git ls-tree`; `records.rs:77` |
| `records.responses` | satisfied | `store.rs:1046`; `records.rs:219` |
| `records.leases` | satisfied | `store.rs:715-736` reads the fetched remote-tracking refs rather than `ls-remote`; see (a)(1) |
| `contract.read` | satisfied | `host.rs:663`; `host.rs:93`; `store.rs:1134` (30 s bound) |
| `contract.write_effect` | **not satisfied** | `store.rs:861-872` misses a repeat inside one tick — finding 12 |
| `contract.lease` | satisfied | `store.rs:906-1019`; `host.rs:157`; `chat.rs:121` |
| `contract.present` | satisfied | `chat.rs:65`; plannotator is out of phase |
| `contract.receive` | satisfied | `records.rs:331`; `served.rs:71`. Discord is group 15; the ✅ reaction is not implemented; a text reply stands in |
| `contract.notify` | **not satisfied** | the 30 s poll and local notify exist (`store.rs:1134`, `disconnected.rs:267`); the git host's push webhook to `/hook` over a Funnel has no route — `http.rs:402-409` |
| `contract.list` | satisfied | as `records.list` |
| `contract.status` | satisfied | `store.rs:1226`; finding 17 |
| `guarantees.durable` | satisfied | `records.rs:61` |
| `guarantees.single_writer` | satisfied | `git.rs:148`; `records.rs:301` |
| `guarantees.atomic` | satisfied | `contract/atomic.yaml` (ign) |
| `guarantees.derivable` | satisfied | `records.rs:281` |
| `guarantees.response_once` | satisfied | `records.rs:219`; `layout.rs:180` |
| `guarantees.disconnected` | satisfied | `disconnected.rs:114`, `:182`; S18 is run by no test |

## `profiles/git-only.yaml` — observations

| entry | class | evidence |
|---|---|---|
| `fetches_per_tick` | scenario-only | the behaviour is right (`host.rs:93`) but the counter resets only in `begin_tick`, which the host never calls — finding 8 |
| `pushes_per_tick` | **not satisfied** | the wording ("one per effect commit") describes neither the host's path nor the runner's — finding 8 |
| `lease_renewals_per_tick` | satisfied | `store.rs:982` (no renewal when none is due); `disconnected.rs:60` |
| `read_processes_per_tick` | **not satisfied** | true of the state store, but `cost()` counts only that repository, and every blueprints read forks `git show` — findings 8, 9 |
| `subprocesses_per_tick` | **not satisfied** | same blind spot |

## `profiles/host.yaml` — Tools

`host.yaml` carries **no `observations:` block** (`grep -c observ` is 0), so
there is nothing to audit there; the cost contract lives in `git-only.yaml` alone.

| entry | class | evidence |
|---|---|---|
| libgit2 through `gix` for reads | **not satisfied** | `flywheel-world-host/src/git.rs:126,136` read with `git show` / `git ls-tree` — finding 9 |
| the `git` binary for merges, rebases and pushes | out of phase | merges and rebases are the effects of 42, bound to the recorded stand-in (93a); the push half is satisfied at `git.rs:148` |
| `wt worktree add/remove` for places | out of phase | no `wt` call anywhere; phase 2 |
| `wt tether` for tethered processes | out of phase | phase 2 |
| portless for ports and local-router endpoints | **not satisfied** | no portless call; the local router is refused (`world.rs:61-70`) and the port-from-place half of 45 has no code — finding 11 |
| mdBook books | out of phase | phase 3 |
| OpenSpec change directories | **not satisfied** | `open_intent` is unbound and `grep openspec/changes crates/*/src` is empty — finding 1 |
| `gh` is not used anywhere | satisfied | the workspace's only `Command::new` calls are two `git` and two binary spawns in the harness; `flywheel/tests/boundaries.rs:10-30` |
| nothing a session leaves on the place's disk is state (67) | satisfied | `flywheel-domain/src/report.rs`; `flywheel/tests/report.rs:33` |
| explicit arguments; the host's own tool configuration never read (183) | satisfied | `git.rs:40,65` pass explicit arg arrays and read no config; no test pins it |
