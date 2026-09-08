# Review: the-loop tasks

**Verdict: revise.** The 114 tasks track the design decision for decision, name all
21 phase-1 scenarios, and carry nothing from phases 2 to 5 — scope is clean, and the
five things the brief asked me to check for (67, 93a, 93b, 253a, 314, D4a, D10a) are
each built by a task. Two structural faults stop it being executable as written.
The scenario runner that every verification depends on is task 11.1, in the
second-to-last group, while roughly twenty tasks in groups 3 to 10 — 3.14 among
them, the step design.md calls the one that admits the profile — verify against a
scenario id or a `contract/` file that only that runner can execute. And no task
creates `flywheel-world-host`, a crate D1's table requires and D8 gives the `World`
trait's only phase-1 implementation, so the manifest-named pair `flywheel host`
loads at 6.8 has no second half and the willdan week of 12.1 has nothing to run on.
Below those: seven spec requirements have no task at all, nine of the thirteen
`contract/` files are named by nothing but 3.14's blanket, three more tasks reach
forward, five tasks are more than one sitting, and about a third of the list
verifies in prose a builder cannot run.

`openspec validate the-loop --strict` from `flywheel-next/main`: `Change 'the-loop'
is valid`.

## Findings, ranked

### 1. The scenario runner is the last thing built and the first thing needed

- **Where:** 11.1, 11.2, against 3.14, 6.3, 6.5, 6.6, 6.15, 7.5, 7.6, 8.4, 8.9,
  9.2, 9.9, 9.10, 10.5, 10.6, 10.7, 10.8, 10.10.
- **Spec or decision:** design.md Migration Plan step 3 ("the thirteen `contract/`
  files go green over the `lamp` machine — this is the step that admits the
  profile"); `conformance/README.md` ("A profile is admitted when `flywheel
  scenario run --profile <name> conformance/` passes"); `scenarios/conformance`
  requirement "A scenario is data, and it lives beside the definitions".
- **Why:** Nineteen tasks state their verification as a scenario id or as
  `contract/*.yaml`. The only mechanism that runs either is `flywheel scenario run`,
  built at 11.1. 3.14 is the worst case: it is group 3's admitting step and it
  cannot be attempted until group 11 exists. This is not a hidden dependency — it
  inverts the stated principle of the file's own header, "each group is provable
  before the next depends on it."
- **Fix:** Move the runner's core — loading a scenario file, seeding `given`,
  running `when` steps, asserting `then` — to group 2, where the stand-in traits it
  runs over are built. Leave in group 11 only what genuinely needs the later work:
  `--hosts real` (11.3), the exclusion subset (11.4), the definitions hash (11.5),
  `render-order` (11.6) and the browser driver (11.7, 11.8). 11.9 stays as the
  whole-suite run.

### 2. No task builds `flywheel-world-host`, and `World` has no real implementation

- **Where:** group 1 (crates), group 5, group 6. No task in the file names the
  crate; `grep -n "world-host" tasks.md` returns nothing.
- **Spec or decision:** D1's crate table (`flywheel-world-host` — "`World` over git,
  the manifest and the host's router; `profiles/host.yaml` is its specification");
  D8's trait table (`World` → `flywheel-world-host` in phase 1, "unchanged" in
  phase 2); 6.8 ("Select the workspace and session bindings from the manifest").
- **Why:** 1.2 declares the `World` trait. 2.2 implements it in `flywheel-scenario`
  as the stand-in. Nothing implements it for a real host. The work that would live
  in it is scattered as behaviour without a home — 5.4 clones repositories, 5.6
  mints installation tokens, 5.7 checks installation coverage, 7.8 asks the router
  for an address — but no task creates the crate or wires `flywheel host` to load
  it. Under AGENTS.md's crate rule that behaviour then widens `flywheel` or
  `flywheel-surface` instead. 12.1 and 12.2 run `flywheel host` against real
  repositories and cannot.
- **Fix:** Add a task in group 5 creating `flywheel-world-host` with `World` over
  git, the manifest and the host's router, and move 5.4, 5.6, 5.7 and 7.8's router
  lookup into it by name. Extend 6.8 to verify the run record names the `World`
  implementation alongside the workspace and session bindings.

### 3. Task 2.3 depends on task 6.10

- **Where:** 2.3 ("playing exits through the reporting command rather than by
  setting evidence; verify the cascade test asserts what the command wrote"),
  against 6.10 ("Implement `flywheel exit | offer | note | refuse`").
- **Spec or decision:** D8 ("the seam is the exit command, which is real and
  identical in both"); `scenarios/conformance` requirement "Only the session
  binding and the recorded effects stand in", scenario "A scripted exit goes
  through the real command" (67, 93).
- **Why:** 2.3 is in group 2 and its verification is that the scripted stand-in
  routes through a command group 6 builds. Group 2's stated purpose is parity with
  no regression, so it cannot be the group that first needs the command.
- **Fix:** Move the exit command (6.10) and its refusal rule (6.11) into group 2,
  ahead of 2.3. Both write through `StateStore::append`, which 1.2 declares and
  2.1 implements, so nothing else moves with them.

### 4. A profile's admission has no task, only the run that assumes it

- **Where:** no task; nearest is 3.14 ("the thirteen `contract/` files").
- **Spec or decision:** `state-store/contract` requirement "A profile is admitted
  only by a complete binding" (138–140, 169, 170) and its four scenarios — an
  incomplete binding refused with the unbound name reported, a guarantee with no
  mechanism refused, the binding naming nothing outside the atoms file, the engine
  calling exactly eight operations; `conformance/contract/binding.yaml`.
- **Why:** This is a whole spec requirement with four scenarios and no task that
  builds it. D3 says `check.py` enforces it, but `check.py` is the model's gate in
  the blueprints repository, and the requirement here is `ADDED` against
  `state-store/contract` — a behaviour of the binary. Nothing refuses an
  incomplete binding at load; nothing reports the unbound name.
- **Fix:** Add a task in group 4 (where definitions load): validate a profile's
  binding at load, refusing one that leaves an evidence or effect name unbound or a
  guarantee without a named mechanism, reporting the name; verify against
  `contract/binding.yaml` and against a deliberately incomplete profile.

### 5. Six spec requirements have no task

- **Where:** none.
- **Spec or decision, one per line:**
  - `organization/bootstrap` — "The new flywheel's objects are its own, in its own
    state repository" (96), scenario "The two do not meet"; D14.
  - `observability/run-record` — "The status view is a projection ... derived from
    list and read alone" (141, 143, 146), scenario "The whole, grouped by state".
    6.14 commits the file and 6.16 fixes drift, but nothing says what it holds.
  - `observability/run-record` — "Discussion about an object is part of its state"
    (144), scenario "A question and its answer stay with the object".
  - `engine/plan` — "A decision exists exactly while its state is active" (9, I3),
    scenario "A decision is retracted when its choice is gone". 10.2 assumes
    retraction ("a re-entered decision state takes a new number") without a task
    building it or the register entry that records the retraction.
  - `state-store/contract` — requirement 1, scenario "No caller reaches storage
    another way" (125): the crates searched for a storage-library dependency. 1.6
    greps for domain names only.
  - `signals/capture-and-curation` — "A signal is immutable and carries what it
    asserts", scenario "An older record reads without conversion" (114, versioned
    and stable signal and move formats).
- **Why:** The brief asks that every spec requirement have a task that builds it
  and one that proves it. These six have neither.
- **Fix:** Six tasks. The coexistence one belongs in group 5 beside 5.9's prefix
  rule; the status view's contents and the discussion record in group 6 beside 6.14;
  retraction in group 10 beside 10.2; the storage grep in group 1 beside 1.6; the
  format-version task in group 9 beside 9.5.

### 6. Two more spec scenarios have no task

- **Where:** none.
- **Spec or decision:** `signals/capture-and-curation` requirement "Curation is a
  bounded judgment", scenario "A proposed intent shows its weight" (109, 118 —
  cites its signals, how many, from which sources, over what span, by event date);
  `hosts/ownership` requirement "More than one host may run for one organization",
  scenario "Two hosts, one plan" (147, 148, 15 — both hosts derive the same plan
  with the same numbers).
- **Why:** 9.11 shows unmoved signals by source and age, which is 118's other half,
  not the proposed intent's weight. 8.1 gives the presenter lease but nothing
  asserts two hosts agree on the plan and its numbers, which is what makes 15's
  "never reused" checkable across hosts.
- **Fix:** Add the weight display to group 9 beside 9.9's S08 run. Add the
  two-host plan-identity assertion to 11.3, where two real processes already exist.

### 7. The state write's own commit is nowhere

- **Where:** 3.3 (write_effect), 3.10 (present and receive), no task for the
  state write.
- **Spec or decision:** D4 second bullet ("**State write**: the new state,
  `entered_at`, counters and the consumed response's id in `applied_responses`, in
  one commit — the same commit — so a response takes effect exactly once");
  `state-store/git-only-profile` requirement "A change of state is a commit",
  scenario "The state write carries the response that caused it" (137, I2);
  `conformance/contract/response-once.yaml`.
- **Why:** 3.3 covers the effect commit and its repeat detection. 3.10 writes the
  response record before the transition fires. Neither is the state commit that
  carries `applied_responses`, which is the mechanism behind the response-once
  guarantee — the thing the proposal calls the point of the phase. The scenario
  format's `applied_responses:` assertion has nothing implementing it.
- **Fix:** A task between 3.3 and 3.4: write the state transition as one commit
  carrying the new state, `entered_at`, the counters and the consumed response id;
  verify a response delivered twice across a restart is applied once, against
  `contract/response-once.yaml`.

### 8. Nine of the thirteen `contract/` files are named by no task

- **Where:** named — 3.3 (`write-effect`), 3.6 (`lease`), 3.8 (`notify`), 3.10
  (`present-receive`). Unnamed — `read`, `list`, `status`, `durable`,
  `single-writer`, `atomic`, `derivable`, `response-once`, `binding`.
- **Spec or decision:** proposal.md — The acceptance ("**`contract/` — all
  thirteen, both paths**"); design.md Goals ("thirteen `contract/` files ... on the
  stand-in and against a local bare state repository").
- **Why:** 3.14's blanket "run the thirteen" is the only mention. A builder working
  3.2 (read and list) or 3.4 (the push) has no file to check against, and four of
  the five guarantees have no task pointing at the scenario that proves them.
- **Fix:** Cite the file in each task that implements its operation or guarantee:
  `read` and `list` on 3.2, `status` on 6.14, `single-writer` on 3.4, `durable` on
  3.1, `atomic` and `response-once` on the new task of finding 7, `derivable` on
  10.1, `binding` on the new task of finding 4. Keep 3.14 as the whole-set run.

### 9. Three more forward dependencies

- **Where and why:**
  - **3.9** — "in-process notification for local causes — a page response, a chat
    message, a session's report" verifies against causes built in groups 6, 7 and 8
    (6.10, 7.9, 8.1). Design D6 and the git-only spec's "A local cause does not wait
    for the poll" name all three.
  - **3.11 and 3.12** — the disconnected rules include "deliver to no sink" (no
    sink until group 8) and 3.12 verifies "against S18", which the git-only spec and
    D15 both say needs two real host processes, built at 11.3.
  - **4.4** — "Load the organization's own type files from the blueprints at the
    shared line" needs the blueprints repository, created by 5.1 in the next group.
- **Fix:** Give 3.9 a verification a group-3 task can run — a synthetic local
  notify through `StateStore` — and add the three real causes as a check on 8.1.
  Split 3.11 and 3.12 so the rules are implemented in group 3 with a single-host
  local check and S18 is asserted in 11.3 with the rest of the two-host set. Either
  move 4.4 after 5.1 or have it read a fixture blueprints directory.

### 10. Five tasks are more than one sitting

- **Where and what to split:**
  - **7.9** — serving the whole page bundle: the plan, the register rendering, the
    status view, the Board, the dock, per-request derivation. Split into the server
    and the plan rendering, the status view rendering, and the dock and Board.
  - **5.1** — four distinct bootstrap effects, each with a proof and each touching
    the git host (create or adopt blueprints, create state, record the App
    requirement, register the first host). One task per effect; keep the
    run-twice-writes-nothing check on the last.
  - **11.3** — the two-host harness plus the lose, disconnect and return steps plus
    three scenarios. Split the harness from the S13, S17 and S18 runs.
  - **7.1** — one function per tool for every operation clause 4 grants; the
    `tools/tool-server` spec lists sixteen by name. Split the catalogue mechanism
    and its schemas from the tool bodies, which 7.4, 7.5 and 7.6 already start.
  - **6.13** — the run record covers four separate spec requirements (79, 80, 81
    and the refusal rule) in one line. One task per requirement.

### 11. About a third of the tasks verify in prose a builder cannot run

- **Where:** 1.3, 1.4, 2.1, 2.2, 2.4, 3.2, 3.7, 3.9, 4.5, 5.2, 5.3, 5.7, 5.11,
  6.4, 6.8, 6.12, 6.16, 7.2, 7.4, 7.11, 7.12, 8.2, 8.5, 8.6, 8.7, 8.8, 9.4, 9.5,
  9.6, 9.7, 9.8, 9.11, 10.2, 10.3, 10.9, 11.4, 12.3.
- **Spec or decision:** AGENTS.md ("`cargo test` is the gate. A scenario under
  `scenarios/` is the acceptance for a change and mirrors a scenario in the model's
  `conformance/`").
- **Why:** These name no `cargo test`, no scenario id and no command, so "verify"
  is a description of an outcome rather than a check anyone can run. The worst are
  **2.4** ("verify each command behaves as before against the stand-in" — no
  baseline is named, and nothing says what "as before" is captured against),
  **6.13** ("verify against `observability/run-record`" — a spec file is not a
  runnable check), and **11.4** ("the others the proposal defers" — the excluded
  set must be enumerated, not gestured at; the proposal's deferral table names
  twenty-four scenarios across five reasons).
- **Fix:** Give each a test name in the crate it touches, or a scenario id, or a
  shell command. For 2.4, name the snapshot the group-2 parity rests on. For 11.4,
  write the excluded list out.

### 12. Three smaller executability gaps

- **Where and what:**
  - **1.6** greps for the seven object names and every string of `atoms.yaml`. The
    spec scenario "The domain boundary is checkable" also searches for instruction
    text (119). Add it.
  - **9.2** implements the meeting-transcript enumerator, but no task names
    `flywheel capture`, which D1 lists as a binary subcommand and the signals spec
    calls "a transcript named to the capture command". Name the command in 9.2.
  - **7.9** and 7.10 build the bundle but nothing verifies D11's "one bundle is
    built and one is served, and its version is the binary's" (307). Add it to 7.9.
  - **8.1** implements the presenter lease. The chat-sink spec's requirement says
    "held by lease or pinned by the manifest — 'pinned' being 148's own word". The
    pinned path has no task; either build it or state in 8.1 that phase 1 binds
    only the lease.

## Keeps

- **Scope is exact.** Nothing from phases 2 to 5 appears: no pane runner, no
  tracker profile, no context map, no interpreter, no hosted tier, no stdio or MCP
  transport. 9.7 explicitly verifies the ledger consequence is *not* attempted
  (116, 101) and 11.4 excludes S14, S32 and X03 by name (93a). 7.7 serves HTTP
  only, as D9 says.
- **Everything the brief named as phase-1-critical is built.** The exit command at
  6.10 and 6.11 (67), the operator runner at 6.9 and 6.12 (93b), the recorded
  workspace at 6.7 and 6.8 (93a), the unsigned-in rule at 7.13 (253a), the 390px
  driver at 11.7 and 11.8 (314), D4a's disconnected rules at 3.11, 3.12 and 3.13,
  and D10a's private-network address at 7.8 and 7.14.
- **All 21 scenarios are named by a task**, each at the group where its behaviour
  is built rather than collected at the end: S01 at 10.5, S02 at 10.6, S04 at 10.7,
  S05 and S06 at 10.10, S07 at 10.8, S08 at 9.9, S13 at 6.5 and 11.3, S16 at 11.2,
  S17 at 11.3, S18 at 3.12 and 11.3, S19 at 3.5, S20 at 6.15, S21 at 8.4 and 9.3,
  S22 at 9.2, S23 and S24 at 9.10, S24 also at 7.6, S29 at 6.6, X01 at 7.5, X05 at
  6.3, X08 at 7.3.
- **Every design decision has a task** except D14: D1 at 1.1–1.6, D2 at 4.1–4.3,
  D3 at 1.2, D4 at 3.3–3.4, D4a at 3.11–3.13, D5 at 3.6–3.7, D6 at 3.8–3.9, D7 at
  6.1, D8 at 2.3 and 6.7–6.12, D9 at 7.1–7.7, D10 at 7.13, D10a at 7.8, D11 at
  7.9–7.11, D12 at 6.14–6.15, D13 at 9.1–9.4, D15 at 11.3 and 11.7–11.8.
- **The two open questions of design.md are carried into the gate.** 12.3 makes the
  willdan week answer the engine windows and the `status.html` cadence and puts the
  answer in the manifest, which is what the design said the week was for.
- **Group 1 precedes everything that uses the crate boundary, and group 2 precedes
  group 3.** Both orderings the brief asked about hold.
- **The three granted clauses are bound where the design says they are.** 6.7 and
  6.8 name the manifest binding for 93a, 6.9 and 6.12 for 93b, 7.13 for 253a.
