Order follows design.md — Migration Plan, with one correction the plan implies:
the scenario runner is built in group 2, because every later group verifies
through it. Spec references are `<capability>` paths under `specs/`; scenario ids
are the model's `conformance/`; `contract/<name>` is a file of
`conformance/contract/`. Every verification is a `cargo test` name, a scenario id
or a command.

## 1. Crates and traits

- [x] 1.1 Add `flywheel-atoms` with the `Evidence` and `Effect` registries generated from `definitions/atoms.yaml` at build time; verify `cargo test -p flywheel-atoms registry_covers_atoms_file` and that adding a name to the atoms file without regenerating fails the build (D1)
- [x] 1.2 Declare `StateStore` (eight methods), `World`, `Workspace` and `Sessions` in `flywheel-atoms` with no implementation; verify `cargo build -p flywheel-atoms` succeeds with `flywheel-engine` as its only workspace dependency (D1, D8, `state-store/contract`)
- [x] 1.3 Move the scenario file types from `flywheel-scenario` into `flywheel-atoms`; verify `cargo test -p flywheel-scenario` passes unchanged
- [x] 1.4 Add `flywheel-domain` with the object envelope and the domain's record schemas over the engine's generic rec reader; verify `cargo test -p flywheel-domain envelope_roundtrip` (D1)
- [x] 1.5 Rename the domain names out of `flywheel-engine`: the `rec.rs:90` fixture and its `claim` fields, the `bolt <name>` fixtures in `tests/guards.rs`, the three `eval.rs` comments, and `let (num, unit)` in the duration parser; verify `cargo test -p flywheel-engine` passes (D1)
- [x] 1.6 Add `cargo test -p flywheel-engine domain_boundary`: grep `crates/flywheel-engine/src/**` and `tests/**` for the seven object names of 86 as whole words, for every string of `atoms.yaml`, and for instruction text; verify it fails on a reintroduced name and passes on the tree (`engine/definitions-and-tick`, 86, 119, I13)
- [x] 1.7 Add `cargo test -p flywheel storage_dependency_boundary`: read the workspace manifests and assert only the state-store crate depends on a git or storage library; verify it fails when the dependency is added elsewhere (`state-store/contract`, 125)

## 2. The exit command, the stand-ins and the scenario runner

- [x] 2.1 Implement `StateStore` in `flywheel-scenario` over the in-memory store; verify `cargo test -p flywheel-scenario store_get_put_append_list`
- [x] 2.2 Implement `flywheel exit done|blocked|stalled`, `flywheel offer`, `flywheel note` and `flywheel refuse`, each writing one thread entry through `StateStore::append`; verify `cargo test -p flywheel exit_writes_one_thread_entry` (67, D8)
- [x] 2.3 Refuse a report that is none of the five exits and record the refusal; verify `cargo test -p flywheel exit_outside_the_five_is_refused` (65, 66, `hosts/ownership`)
- [x] 2.4 Implement the scripted `Sessions` stand-in, playing every exit, offer and refusal by running the command of 2.2 rather than by setting evidence; verify `cargo test -p flywheel-scenario scripted_exit_goes_through_the_command` asserts the thread entry the command wrote (67, 93, `scenarios/conformance`)
- [x] 2.5 Implement the stand-in `World` and `Workspace` from the existing `world.rs` simulation; verify `cargo test -p flywheel-scenario` passes unchanged
- [x] 2.6 Implement the scenario runner's core — load a scenario file, validate it against `conformance/schema.json`, seed `given` (objects, evidence, register, marks, script, files, hosts), run the `when` steps (`tick`, `response`, `evidence`, `script`, `notify`, `restart`, `direct`, `files`, `clock`, `host`), and assert `then`; verify `flywheel scenario run conformance/scenarios/S01.yaml` passes (94, `scenarios/conformance`)
- [x] 2.7 Evaluate every `then` clause the suite uses — `transitions`, `effects` with counts, `decisions` with numbers and presence, `writes`, `applied_responses`, `states`, `leases`, `status` and `state_store` — failing on an unknown key; verify `cargo test -p flywheel-scenario expect_keys_are_exhaustive` (94)
- [x] 2.8 Read each profile's `observations:` block as a named registry, resolve every `then.state_store` key through the registry of each profile the scenario applies to, and fail with exit 2 on a key no such profile binds; verify `cargo test -p flywheel-scenario unbound_observation_is_an_error` asserting an unbound key fails rather than passing silently, and `observations_are_parameterized` asserting no observation name holds a host name (94, D15)
- [x] 2.9 Implement `--trace [dir]`, defaulting to `target/flywheel-trace/<profile>/` and never writing beside the scenario files: write `<name>.trace.json` as the structured record the assertions themselves evaluate and render `<name>.trace.md` from it, listing the ticks, guards, transitions, effects, decisions and their numbers; verify `flywheel scenario run conformance/scenarios/S16.yaml --trace` writes both files under the default directory, `cargo test -p flywheel-scenario trace_lists_all_six`, and `trace_md_is_rendered_from_trace_json` asserting the document and the evidence cannot diverge (95, D15)
- [x] 2.10 Make the scenario clock virtual, moving for exactly two reasons — a `clock` step and each `tick` step by one tick interval, declared per run and defaulting to D7's 60-second sweep — behind an injected clock source no guard can reach past; verify `cargo test -p flywheel-scenario clock_moves_only_on_clock_and_tick` and `wall_clock_never_reaches_a_guard` asserting an `older:` guard fires on the virtual clock alone (D15, D7)
- [x] 2.11 Close the host step's vocabulary in `conformance/schema.json` and honour it: `{name}` alone sets the acting host, at most one of `start | lose | disconnect | return` performs a transition, a scenario with no host step runs as a single host named `local`, concurrency is `tick: {concurrent_hosts: [a, b]}`, and `bypass_lease` is a declared contract-only hook honoured in process alone; verify `cargo test -p flywheel-scenario host_step_vocabulary_is_closed` asserting a step naming two transitions fails the schema, `no_host_step_runs_as_local`, and `bypass_lease_refused_outside_in_process` (232, D15)
- [x] 2.12 Give the `direct` step one required discriminator in the schema — `direct: {do: adapter | commit | board | close, …}`, each arm with its own required fields; verify `cargo test -p flywheel-scenario direct_arms_require_their_fields` asserting a mistyped key fails the schema with exit 2 rather than being ignored (D15)
- [x] 2.13 Resolve every path in `given.files`, in a `files` step and in a `direct` argument relative to `conformance/fixtures/`, and materialize a short file given inline as path-to-content into the scenario's temporary checkout; verify `cargo test -p flywheel-scenario fixture_paths_resolve_under_fixtures` and that a path naming no fixture fails with exit 2 (D15)
- [x] 2.14 Resolve `response.decision` through the register at the moment the step runs, failing the scenario when it names no standing decision, and keep the `number` form for the answer-it-again assertions; verify `cargo test -p flywheel-scenario decision_id_resolves_through_the_register` and `decision_naming_nothing_standing_fails` — the prototype's silent ignore of the id form is what this closes (15, D15)
- [x] 2.15 Add `requires: [real-workspace | real-sessions]` to the schema and skip a scenario whose requirements the bound implementations do not provide, printing it as skipped with the reason; verify `cargo test -p flywheel-scenario requires_is_read_from_the_data` asserting the skip is decided by the scenario file and by no list held outside it, and that a scenario the acceptance table lists which every configuration skips is reported as a failure (93a, D15)
- [x] 2.16 Report a run: one line per scenario then a summary, exit 0 all passed, 1 an assertion failed, 2 a scenario invalid against the schema or using an unbound name, 3 the profile refused for an incomplete binding or an unmatched definitions hash; a failure prints the scenario and step, the clause numbers from `satisfies:`, expected against actual, and the trace path; verify `cargo test -p flywheel-scenario exit_codes_are_four` and `failure_line_names_clauses_and_trace` (79–82, 167, D15)
- [x] 2.17 Move `seed`, `tick`, `respond`, `dictate`, `rail` and `log` onto the traits; verify `cargo test -p flywheel cli_snapshot` against a committed snapshot of each command's output over `scenarios/rail-mockup.yaml`, captured before the move

## 3. The git-only state store

- [x] 3.1 Add `flywheel-store-git` with the state repository's layout — a directory per object, the response, ask and run-record paths, the lease and host branch names; verify `flywheel scenario run --profile git-only conformance/contract/durable.yaml` passes against a local bare repository (160, `state-store/git-only-profile`)
- [x] 3.2 Implement `read` and `list` over the fetched shared line, with `git diff --name-only` naming what moved; verify `flywheel scenario run --profile git-only conformance/contract/read.yaml conformance/contract/list.yaml` (126, 131, 166)
- [x] 3.3 Implement `write_effect`: one commit per effect carrying its identity, reason and evidence, with repeat detection over the fetched history before committing, so the write is the no-op and the act still runs when its proof is absent; verify `flywheel scenario run --profile git-only conformance/contract/write-effect.yaml` (73, 127)
- [x] 3.4 Write a state transition as one commit carrying the new state, `entered_at`, the counters and the consumed response's id in `applied_responses`; verify `flywheel scenario run --profile git-only conformance/contract/atomic.yaml conformance/contract/response-once.yaml` and that a response delivered twice across a restart is applied once (137, I2, D4)
- [x] 3.5 Implement the push with expected-old and the rebase-retry, reporting and re-reading after three rejections; verify `flywheel scenario run --profile git-only conformance/contract/single-writer.yaml` (134, 162)
- [x] 3.6 Treat a rebase that conflicts on content as a loss — discard the local commit, re-read, take the operator's commit as the response; verify `flywheel scenario run --profile git-only conformance/scenarios/S19.yaml` (3, 164, I15)
- [x] 3.7 Implement `lease` as pushes to a branch per object with expected-old, with the 5-minute stale window and the 24-hour expiry; verify `flywheel scenario run --profile git-only conformance/contract/lease.yaml` (128, 163)
- [x] 3.8 Implement the heartbeat branch per host; verify `cargo test -p flywheel-store-git renewals_add_no_commit_to_main` simulating a month of renewals and asserting `git log main` is unchanged (163, 167)
- [x] 3.9 Implement `notify` as the 30-second bounded poll of the shared line's head, with the branch-move call as an opt-in the manifest names; verify `flywheel scenario run --profile git-only conformance/contract/notify.yaml` (130, 166)
- [x] 3.10 Accept an in-process notify for an object and tick it without waiting for the poll; verify `cargo test -p flywheel-store-git local_notify_ticks_at_once` with a synthetic cause raised through the store (130, D6) — the three real causes are checked at 8.10
- [x] 3.11 Implement `present` and `receive`, writing the response record before the transition fires and acknowledging the operator; verify `flywheel scenario run --profile git-only conformance/contract/present-receive.yaml` including the unapplicable path (129, 153, 154)
- [x] 3.12 Implement the disconnected rules of D4a — keep ticking owned objects to the expiry, commit locally, take no lease, start nothing new, deliver to no sink — and report a write made while disconnected as pending; verify `cargo test -p flywheel-store-git disconnected_rules` with one host and a cut route (151, 165)
- [x] 3.13 Implement the reconnect order — renewals first, discard local commits on an object whose lease was lost, then rebase and push; verify `cargo test -p flywheel-store-git reconnect_pushes_renewals_first` — S18 is asserted at 11.2 with two host processes
- [x] 3.14 Push unpushed commits found at start, before the first tick, discarding those on an object no longer held and reading nothing from the working tree; verify `cargo test -p flywheel-store-git unpushed_commits_at_start` (I14, 165)
- [ ] 3.15 Run the whole contract set against a local bare repository: `flywheel scenario run --profile git-only conformance/contract/`; verify all thirteen pass — this is the step that admits the profile (168)

## 4. Definitions embedded and the binding gate

- [x] 4.1 Embed `definitions/` in the binary as core definitions and expose the set version; verify `flywheel version --definitions` prints the set version and the version of every core machine (223, 224)
- [x] 4.2 Add `cargo test -p flywheel-domain embedded_set_matches_definitions_dir`, hashing the embedded set against `definitions/`; verify it fails when the directory is edited without a rebuild (D2)
- [x] 4.3 Accept `--definitions <dir>` on the scenario runner and refuse it on `flywheel host`; verify `cargo test -p flywheel host_refuses_definitions_override` (D2)
- [x] 4.4 Validate a profile's binding at load: refuse one that leaves an evidence or effect name unbound or a guarantee without a named mechanism, refuse a binding naming evidence outside the atoms file, and report the name; verify `flywheel scenario run --profile git-only conformance/contract/binding.yaml` and `cargo test -p flywheel-domain incomplete_binding_is_refused` against a deliberately incomplete profile (138–140, 169, 170, `state-store/contract`)
- [x] 4.5 Load the instance's own type files from a blueprints checkout at the shared line; verify `cargo test -p flywheel-domain type_from_blueprints_runs` against a fixture blueprints directory, with no binary change and no host restart (57, 85)
- [x] 4.6 Record the extensible machine's version on each object and hold it across a type change; verify `cargo test -p flywheel-domain type_version_held_in_flight` (57, 224)
- [x] 4.7 Refuse a blueprints file that would override a core machine and write the refusal to the run record; verify `cargo test -p flywheel-domain core_machine_override_refused` (223)

## 5. The world, bootstrap and host join

- [x] 5.1 Add `flywheel-world-host` implementing `World` over git, the manifest and the host's router, with `profiles/host.yaml` as its specification; verify `cargo test -p flywheel-world-host` covers a repository read, a manifest read and a router lookup (D1, D8)
- [x] 5.2 Implement create-or-adopt of the blueprints repository from its template, with its proof; verify `cargo test -p flywheel-world-host adopt_is_idempotent` against a sandbox git host (204)
- [x] 5.3 Implement creation of the state repository with the profile's layout, with its proof; verify `cargo test -p flywheel-world-host create_state_repository` (204, C.2)
- [x] 5.4 Record that the App must be installed and raise the attention decision while it is unseen; verify `cargo test -p flywheel init_awaits_app_install` asserting the decision stands and no agent acts (204, 207, 82)
- [x] 5.5 Register the first host and complete the machine; verify `cargo test -p flywheel init_twice_writes_nothing` and `init_resumes_half_finished` (204)
- [x] 5.6 Implement `flywheel host join` in `flywheel-world-host`: bare clones of the state, the blueprints and every tracked repository under the manifest's root, one checkout per shared line, no worktree; verify `cargo test -p flywheel-world-host join_clones_only_what_is_missing` (205, 93a)
- [x] 5.7 Implement `flywheel host doctor` and run it at join and every tick; verify `cargo test -p flywheel-world-host doctor_names_first_difference` against a hand-made path (205, 222)
- [x] 5.8 Read the App key from where the operator placed it and mint short-lived installation tokens, in `flywheel-world-host`; verify `cargo test -p flywheel-world-host token_written_nowhere_else` asserting no key or token in configuration, code or the state repository (207, 207a)
- [x] 5.9 Raise the attention decision for a manifest repository the App's installation does not cover, in `flywheel-world-host`; verify `cargo test -p flywheel-world-host uncovered_repository_decision` (207)
- [x] 5.10 Enforce the prefix rule: refuse and report a tracked write outside the machinery's prefix that is not the effect of a response; verify `cargo test -p flywheel prefix_write_refused` (203)
- [x] 5.11 Scope the new flywheel to its own state repository and prefix, touching nothing of the existing flywheel's; verify `cargo test -p flywheel coexistence_scope_is_disjoint` asserting no read or write outside them (96, D14, `instance/bootstrap`)
- [x] 5.12 Implement the instance's removal by dictation — sessions ended, places removed, state archived with its counter, repositories left on disk; verify `cargo test -p flywheel remove_instance_keeps_counter` asserting no number is reused (221, 15, 4)
- [x] 5.13 Hold a repository record to its git details alone; verify `cargo test -p flywheel repository_record_has_git_details_only` refusing a kind, capability or scope (199)
- [x] 5.14 Stamp the set version at initialization and at repository creation; verify `cargo test -p flywheel set_version_stamped` and that a newer set upgrades nothing (208)

## 6. The host loop, the workspace and the sessions

- [x] 6.1 Implement `flywheel host` as one long-lived process with the notify-tick and the 60-second sweep, fetching before every tick; verify `cargo test -p flywheel sweep_fires_older_guards` and `tick_fetches_first` (D7, 165, 231)
- [x] 6.2 Implement the host's declaration, heartbeat and lease-taking within it; verify `cargo test -p flywheel lease_only_within_declaration` (149)
- [x] 6.3 Raise and clear the uncovered attention decision; verify `flywheel scenario run conformance/scenarios/X05.yaml` (149, 150)
- [x] 6.4 Implement the intermittent host's away window — away with since-when, leases standing, stall clocks paused, no attention line, takeover raised only when work waits or the long bound passes; verify `cargo test -p flywheel away_raises_no_attention` and `away_with_work_waiting_raises_takeover` (150a)
- [x] 6.5 Implement takeover: a fresh attempt on the taking host, the returning host ending its own session and reporting; verify `cargo test -p flywheel takeover_starts_attempt_two` — S13 is run at 11.2 with two processes
- [x] 6.6 Implement the per-host session bound with the stated waiting order and the dependency gate; verify `flywheel scenario run conformance/scenarios/S29.yaml` (31, 32, 38)
- [x] 6.7 Add `flywheel-workspace-recorded` implementing `Workspace` by writing the evidence each proof reads; verify `cargo test -p flywheel-workspace-recorded place_advances_without_a_repository` (93a, D8)
- [x] 6.8 Select the `World`, `Workspace` and `Sessions` implementations from the manifest and record all three in the run record; verify `cargo test -p flywheel bindings_named_in_run_record` (93a, 139)
- [x] 6.9 Add `flywheel-sessions-operator`: `start_session` records the session with its place and work order and starts no agent, and the rail and status view show it as the operator's to run; verify `cargo test -p flywheel-sessions-operator no_agent_started` (93b, 89)
- [x] 6.10 Apply the with-operator rules to an operator-bound session: no finish-or-keep on idle, ends only by dictation; verify `cargo test -p flywheel-sessions-operator idle_offers_nothing` over a simulated day (93b, 25)
- [x] 6.11 Record every write with its reason and the evidence the guard read; verify `cargo test -p flywheel run_record_carries_reason_and_evidence` (79, `observability/run-record`)
- [x] 6.12 Record what was expected of a session beside what it delivered, difference first; verify `cargo test -p flywheel expected_beside_delivered` with a session delivering two of three (80)
- [x] 6.13 Report a problem with the machinery through the run record and create no work for it; verify `cargo test -p flywheel machinery_problem_is_not_work` (81)
- [x] 6.14 Record every refusal with the identity, the operation and the object, and surface it under attention; verify `cargo test -p flywheel refusal_reaches_attention` (4, 79, 81)
- [x] 6.15 Derive the status view from `list` and `read` alone: every object grouped by queued, in progress, waiting on the operator and done, with its holder, its runner and that host's liveness, one place for the instance; verify `cargo test -p flywheel status_view_groups_every_object` (141, 143, 146)
- [x] 6.16 Keep a question, an answer and a note on the object and show them under it on the status view; verify `cargo test -p flywheel discussion_stays_with_the_object` reading them back after the session is gone (144)
- [x] 6.17 Implement `render_status` as an effect of the rail object, committing the status file on the shared line with its as-of commit and time, written only by the rail's lease holder; verify `flywheel scenario run --profile git-only conformance/contract/status.yaml` (D12, 132, 145, 148)
- [x] 6.18 Verify S20: `flywheel scenario run --profile git-only conformance/scenarios/S20.yaml` — the committed file is readable from the state repository alone and its as-of commit is the last that landed
- [x] 6.19 Rewrite a drifting projection from its source on the next tick and report both values; verify `cargo test -p flywheel drift_rewritten_and_reported` (77, 142)

## 7. The tool catalogue and the page

- [x] 7.1 Implement the catalogue mechanism: one registry, a schema per tool naming its arguments by object id, and the same enumeration in-process and over HTTP; verify `cargo test -p flywheel-surface catalogue_is_identical_across_callers` (193, `tools/tool-server`)
- [x] 7.2 Implement the tool bodies for the sixteen operations the spec names, each writing through the state store; verify `cargo test -p flywheel-surface every_named_tool_has_a_body` asserting the catalogue and the spec's list agree (193)
- [x] 7.3 Record every call once as a response carrying the tool, the object, who gave it and when; verify `cargo test -p flywheel-surface call_recorded_once` with a call delivered twice (153, 137)
- [x] 7.4 Omit every tool that would assert work was done and record an arriving claim as unapplicable under attention; verify `flywheel scenario run conformance/scenarios/X08.yaml` (4, 6)
- [x] 7.5 Implement the undo-or-defer dictation verbs plus service start and stop, each taking the transition its decision would; verify `cargo test -p flywheel-surface dictation_takes_the_decisions_transition` (4, 12)
- [x] 7.6 Implement `open-session`; verify `flywheel scenario run conformance/scenarios/X01.yaml` (69)
- [x] 7.7 Implement `revive`; verify `flywheel scenario run conformance/scenarios/S24.yaml` (107, 12)
- [x] 7.8 Serve the catalogue over HTTP for the page; verify `cargo test -p flywheel-surface http_call_writes_the_same_record` as the in-process caller (193)
- [x] 7.9 Bind the host's address to the private-network router's name from the manifest, through `flywheel-world-host`'s router lookup, keeping the localhost port for the operator at the machine; verify `cargo test -p flywheel-world-host address_is_the_routers_name` and `cargo test -p flywheel-surface link_never_names_localhost` (191, 205a, 308, D10a)
- [x] 7.10 Serve the page and render the rail from the register and the objects on each request, storing no rendering; verify `cargo test -p flywheel-surface rail_rendered_per_request` (15, `surfaces/page`)
- [x] 7.11 Render the status view on the page from the same read; verify `cargo test -p flywheel-surface page_carries_status_view` (141, 310)
- [x] 7.12 Lay the bundle out under 760px as Decisions and Board with the dock full screen and a back control; verify `cargo test -p flywheel-surface layout_switches_at_760px` (307)
- [x] 7.13 Build and serve one bundle whose version is the binary's, holding no client state a reload loses and fetching nothing external; verify `cargo test -p flywheel-surface one_bundle_version_is_the_binarys` and `bundle_has_no_external_fetch` (307, 310)
- [x] 7.14 Give each kind its one form and open an elaboration from its intent; verify `cargo test -p flywheel-surface only_a_decision_is_answerable` (209, 210)
- [x] 7.15 Implement the capture box and the mark-as-intent control; verify `cargo test -p flywheel-surface capture_box_parses_nothing` with text beginning in a command-like word (19, 194)
- [x] 7.16 Serve unsigned-in while the operators list holds one entry, recording that entry as `given_by`, and refuse on a second operator or another address; verify `cargo test -p flywheel-surface unsigned_in_single_operator` and `refuses_on_second_operator` (253a, 153, 236a)
- [x] 7.17 Bind the host to its private-network address and the localhost port and to nothing else; verify `cargo test -p flywheel-surface binds_two_addresses_only` (46, 155, 245)

## 8. The chat sink

- [ ] 8.1 Implement the Discord sink and take its presenter lease, phase 1 binding the lease alone and no manifest pin; verify `cargo test -p flywheel-surface one_presenter_per_sink` (148, `surfaces/chat-sink`)
- [ ] 8.2 Render one line per decision with its number and a link, keeping each kind's form; verify `cargo test -p flywheel-surface chat_and_page_show_one_number` (15, 18)
- [ ] 8.3 Accept the numbered reply grammar as the answer tool and expand "yes all" into one response per decision; verify `cargo test -p flywheel-surface yes_all_expands_per_decision` with distinct delivery identities (11, 137, 194)
- [ ] 8.4 Accept a forwarded message as a capture; verify `flywheel scenario run conformance/scenarios/S21.yaml` (112, 215)
- [ ] 8.5 Reply to any other message with what the sink accepts and write nothing; verify `cargo test -p flywheel-surface free_text_writes_nothing` (194)
- [ ] 8.6 Carry the platform's answer controls and the link on the posted message, with a numbered reply answering the same decision; verify `cargo test -p flywheel-surface posted_message_carries_controls_and_link` (309, 155)
- [ ] 8.7 Advance the sink's mark in the same write as its delivery; verify `cargo test -p flywheel-surface mark_advances_with_delivery` and that each sink's tail is its own (14, 236)
- [ ] 8.8 Route notifications by kind to the sinks the operator sets; verify `cargo test -p flywheel-surface routed_kind_reaches_its_sinks` with the event raised on a host presenting no sink (82)
- [ ] 8.9 Say so when a link points at an away host; verify `cargo test -p flywheel-surface away_link_says_so` (308, 150a)
- [ ] 8.10 Raise the in-process notify from the three real local causes — a page response, a chat message, a session's report; verify `cargo test -p flywheel local_causes_tick_at_once` for each (130, D6)

## 9. The adapters, signals and curation

- [ ] 9.1 Write captures keyed by source event with their provenance and pointer, under the machinery's blueprints prefix, the raw material left outside; verify `cargo test -p flywheel capture_keyed_once` (111, 203)
- [ ] 9.2 Implement the meeting-transcript enumerator behind `flywheel capture meeting <file>`; verify `flywheel scenario run conformance/scenarios/S22.yaml` — a second import writes nothing and starts no session (111, 115)
- [ ] 9.3 Implement the chat-forward enumerator; verify `cargo test -p flywheel-surface forward_writes_capture_and_signal` (112, 215)
- [ ] 9.4 Write the page capture box's single ask signal; verify `cargo test -p flywheel-surface box_writes_one_ask_signal` (19)
- [ ] 9.5 Write signal records with their kind, asserter, subject, assertion and verbatim excerpt, and expose no tool that edits one; verify `cargo test -p flywheel signal_is_never_rewritten` reading the file's history (113, 193)
- [ ] 9.6 Version the signal and move record formats and read an earlier version without conversion; verify `cargo test -p flywheel older_signal_reads_unconverted` against a fixture written in the earlier format (114)
- [ ] 9.7 Write move records with the signal id, target, reason and date, one standing move per signal, curation seeing only unmoved ones; verify `cargo test -p flywheel one_standing_move_per_signal` (107)
- [ ] 9.8 Implement the move consequences, a challenge recording the claim by name and version and attempting no ledger effect; verify `cargo test -p flywheel challenge_records_claim_only` (116, 101)
- [ ] 9.9 Charge curation on its cadence and on the unmoved threshold, catching a missed cadence up once under the idempotent key; verify `cargo test -p flywheel curation_cadence_caught_up_once` over a simulated day down (110, 231, 111)
- [ ] 9.10 Verify S08: `flywheel scenario run conformance/scenarios/S08.yaml` — one move each, joins become proposed intents, one decision per proposed intent
- [ ] 9.11 Show a proposed intent's weight: its signals, how many, from which sources and over what span, counted by event date; verify `cargo test -p flywheel-surface proposed_intent_shows_weight` (109, 118)
- [ ] 9.12 Verify S23 and S24: `flywheel scenario run conformance/scenarios/S23.yaml conformance/scenarios/S24.yaml`
- [ ] 9.13 Show unmoved signals by source with their age on the status view and discard none; verify `cargo test -p flywheel unmoved_signals_by_source` (118)

## 10. The rail

- [ ] 10.1 Derive the rail from active states and the register on every tick; verify `flywheel scenario run --profile git-only conformance/contract/derivable.yaml` (7, `engine/rail`)
- [ ] 10.2 Create a decision when its state becomes active and retract it when the state is left, writing the retraction on its register entry; verify `cargo test -p flywheel-engine decision_created_and_retracted` (9, I3)
- [ ] 10.3 Number decisions in one atomic write of the rail record, never reusing a number, a re-entered state taking a new one; verify `cargo test -p flywheel-engine number_never_reused` (15)
- [ ] 10.4 Group and fold decisions so "yes to all" is meaningful and any one answers alone; verify `cargo test -p flywheel-engine grouping_and_single_answer` (11)
- [ ] 10.5 Derive the tail from each sink's mark with no rendering stored; verify `cargo test -p flywheel-engine tail_per_sink_mark` asserting the page's and the chat's differ (14, 15)
- [ ] 10.6 Hold an intent to one elaboration awaiting approval and let the response correct its type; verify `cargo test -p flywheel-engine new_material_joins_the_proposal` and `response_corrects_the_type` (21, 27)
- [ ] 10.7 Verify S01: `flywheel scenario run conformance/scenarios/S01.yaml`
- [ ] 10.8 Verify S02: `flywheel scenario run conformance/scenarios/S02.yaml`
- [ ] 10.9 Verify S04: `flywheel scenario run conformance/scenarios/S04.yaml`
- [ ] 10.10 Verify S07: `flywheel scenario run conformance/scenarios/S07.yaml`
- [ ] 10.11 Verify S05 and S06 against the real store: `flywheel scenario run --profile git-only conformance/scenarios/S05.yaml conformance/scenarios/S06.yaml`

## 11. Two hosts, the hash, the prompt and the phone

- [ ] 11.1 Implement `--hosts real`: every host of `given.hosts` is a child of `std::env::current_exe()` with `--root <tmp>/<name>` and a port range from the router, the runner holds no engine and asserts only through the store, and the `start`, `lose`, `disconnect` and `return` steps drive those processes; verify `cargo test -p flywheel-scenario two_host_harness` starts, loses and returns one, and `runner_holds_no_engine_under_hosts_real` (232, 134, 162, I15, D15)
- [ ] 11.1a Take the child hosts through the same virtual clock by an injected clock source and trigger their sweep from the runner rather than from a timer; verify `cargo test -p flywheel-scenario two_host_run_is_deterministic` running S13 ten times under `--hosts real` and asserting one trace (D15, D7)
- [ ] 11.2 Verify S13, S17 and S18 with two host processes: `flywheel scenario run --hosts real --profile git-only conformance/scenarios/S13.yaml conformance/scenarios/S17.yaml conformance/scenarios/S18.yaml`
- [ ] 11.3 Assert both hosts derive the same rail with the same numbers; verify `cargo test -p flywheel-scenario two_hosts_one_rail` under the harness of 11.1 (147, 148, 15)
- [ ] 11.4 Set `requires: [real-workspace]` on exactly the scenarios asserting a real take, merge, rebase, conflict or landing — S12, S14, S15, S26, S32, S33, S34, X03, X06, X09 — so a recorded-workspace run skips them from the data of 2.15 and names both the ran subset and the skipped set with its reason in the run record; verify `cargo test -p flywheel-scenario recorded_workspace_excludes_ten` asserting exactly those ten are skipped and that the other thirteen deferrals (S03, S28, S30, S31, X07, T01, S09, S10, S11, S25, S27, X02, X04) are excluded for their own reasons (93a, D15)
- [ ] 11.5 Record the machine files' hash in the run record and compare it to the definitions directory; verify `cargo test -p flywheel-scenario run_record_hash_matches_definitions` fails on a mismatch (168, D2)
- [ ] 11.6 Implement `flywheel render-order <session type> <instruction version> <scenario>` rendering the prompt with no session started; verify `cargo test -p flywheel render_order_inputs_are_closed` asserting the schema instruction, the type skill, the work order and the change's artifacts and nothing else, and `render_order_two_versions_differ` (88, 89, 90, 123, 124)
- [ ] 11.7 Add the headless Chromium driver to `flywheel-scenario` as a dev-dependency of that crate and of no shipped crate, serving the page from the process that just ticked and opening the rail at a chosen viewport to tap a control; verify `cargo test -p flywheel-scenario driver_taps_at_390px` and `cargo test -p flywheel driver_is_no_shipped_dependency` reading the workspace manifests (314, D15)
- [ ] 11.8 Have the runner select the 390px set itself — every scenario with a `response` step, so no list is kept by hand — find the decision by the number the register gave it, and assert at 390×844 and again at 1440×900 that the number and its answers are reachable by tap with nothing behind a hover or a keyboard, that the answer is posted through the same tool the reply grammar calls, and that a reload shows `given_by` and `given_at`; a scenario adds a selector only if it needs one. Verify `cargo test -p flywheel-scenario phone_pass_for_every_response_scenario` and `phone_set_is_selected_not_listed` (311, 193, 310, 153, 314, D15)
- [ ] 11.8a Write a run record through the store for every scenario run, holding the profile, the definitions hash, the scenarios that ran, the subset skipped with its reason and the failures; verify `cargo test -p flywheel-scenario run_record_names_ran_and_skipped` (79–82, 93a, 167, D15, `observability/run-record`)
- [ ] 11.9 Run the whole phase-1 set: `flywheel scenario run conformance/` on the stand-in and `flywheel scenario run --profile git-only conformance/` against a local bare repository; verify the thirteen contract files and the twenty-one scenarios all pass

## 12. The phase gate

- [ ] 12.1 Initialize the willdan instance on the git-only profile and join the laptop host; verify `flywheel host doctor` passes and the instance's record reads bootstrapped (204, 205)
- [ ] 12.2 Run the loop on real work for a week — captures landing, decisions answered from the phone, responses recorded, the record readable; verify the week completes and `git log` on the state repository shows no commit made by hand (roadmap, phase gates)
- [ ] 12.3 Set the engine windows and the status file's rewrite cadence in `flywheel.yaml` from what the week measured, or record that the defaults held; verify `flywheel host` reads the values back at load (design.md — Open Questions)
- [ ] 12.4 Confirm the gate: `cargo test`, `flywheel scenario run --profile git-only conformance/`, and `uv run --with pyyaml --with jsonschema python3 machines/check.py` in the model with `definitions/` byte-identical
