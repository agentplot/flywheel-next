# `flywheel scenario run` — what it is, how phase 1 uses it, what is still open

## 1. What it is

`flywheel scenario run` is the conformance runner: it loads the shipped
definitions, seeds a described state of the stores, plays a scenario's `when`
steps against the real engine, and asserts the `then` clauses. A scenario is
data — given this evidence, when this tick or event, then these transitions,
these effects and these decisions (94). The same runner, the same files and
byte-identical machine
files run against the stand-in state store and against the git-only profile;
only the store binding, the session binding (93) and, in phase 1, the
line-and-place binding (93a) differ. That is why it is the phase gate: a
profile is admitted only when it binds every evidence and effect name the
definitions use, satisfies every operation of B.1 with every guarantee of B.2,
and passes the suite unchanged (168), and a phase ends when its scenarios pass
in conformance and the willdan operator has run a week on it (roadmap, Phase
gates). Everything the machinery owns — the store, the engine, the plan, the
page, the register — runs for real; nothing is asserted about the script.

## 2. A worked walk

### S01 — one response starts work, nothing re-asks

`models/statechart/conformance/scenarios/S01.yaml`, `profiles: [all]`,
`satisfies: [1, 6, 13, 24, 314]`.

```bash
flywheel scenario run conformance/scenarios/S01.yaml --trace
```

**Load.** The binary's embedded definition set is used unless `--definitions`
names a directory (D2). Its hash is written into the run record and compared
with `definitions/` on disk (168, task 11.5).

**Bind.** Default: the stand-in `StateStore` and `World` in
`flywheel-scenario`, the recorded `Workspace` (93a, D8), and the scripted
`Sessions` stand-in from `profiles/sessions-stand-in.yaml`, which replaces
`profiles/sessions.yaml`. With `--profile git-only`: the same run against a
temporary bare `flywheel-state` repository on disk, no network, everything
else unchanged.

**Seed.** Two objects — an open `intent` and an `elaboration` in `proposed`
carrying `{type: self-closing, type_version: 1}` — plus the given evidence
(`line.contains_parent` true, every `session.pane` absent). Anything the
scenario does not list is absent, false or none. `given.files` would
materialize blueprints or built-repository files the world reports; S01 needs
none.

**Play, step by step.**

| # | step | what happens |
|---|---|---|
| 1 | `tick` | `elaboration.proposed` derives `elaboration-proposed`, numbered by the register |
| 2 | `response` | delivery `discord/1001` names the decision by id, resolved through the register to its number; `{response: yes}` fires `proposed → approved`, `applied_responses += discord/1001`, the decision retracts |
| 3–4 | `tick` | `approved → placing` (parent in open); `place: absent → preparing` (`prepare_place`) |
| 5 | `evidence` | the world changed: `place.exists`, `place.contains_line`, `place.ready` all true |
| 6–7 | `tick` | `placing → working`; `self-closing.session: requested → starting` (`start_session`) `→ alive` |
| 8 | `response` | the same delivery id again — already in `applied_responses`, no guard matches, nothing re-asks (I2) |
| 9 | `evidence` | the pane is present and working |
| 10 | `restart` | every in-memory thing is dropped |
| 11 | `tick` | `working` is re-derived from the record and the pane, not from memory (75, I14) |

The operator's answer at step 2 goes through the same tool the page's control
calls (193): the runner does not poke state, it calls `StateStore::receive`
with the delivery, exactly as the Discord sink or the page would. A scripted
session exit, where a scenario has one, is played by the stand-in running the
real `flywheel exit` command, which writes through `StateStore::append` (67,
D8); the assertion is about the thread entry the command wrote.

**Evaluate.** `transitions` in order across all ticks; `effects` by atom name
with a count (`prepare_place` 1, `start_session` 1 — a repeat must not add);
`decisions` at named step boundaries (present after 1, absent after 3, none
after 11); `applied_responses`; final `states`. `state_store:` — S01 has none —
carries assertions about the profile's own behavior, and is where the contract
files live: `write-effect.yaml` asserts `{writes_with_effect_id: 2,
distinct_effect_ids: 1, second_reported_as_write: false}`, which on git-only
means two commits carrying the same effect id in their message, the second
found by `git log --grep` and changing nothing.

**Trace.** `--trace` renders the run as a document a person reads (95): the
ticks, the guards that matched, the transitions, the effects and the decisions
with their numbers.

### S13 — two hosts, one loses power

`profiles: [all]`, `satisfies: [147, 150]`, `invariants: [I11]`. Two host
objects and a lease held by `mac-mini` over a work item mid-build.

```bash
flywheel scenario run --profile git-only --hosts real conformance/scenarios/S13.yaml
```

`--hosts real` starts each named host as its own `flywheel host` process with
its own root, multiplexer session and port range (232, D15). Step 1
`host: {name: mac-mini, lose: true}` stops that process without a farewell, so
its heartbeat stops. Step 2 makes `studio` the acting host. The clock advances
6m, a tick makes `host/mac-mini` stale and its lease stale, and the status view
shows both as stale. 25m more and a tick takes the host to `gone`, raising
`host-gone`. The operator answers `takeover`; the lease expires, `studio` starts
its attempt. When `mac-mini` returns it ends its own pane and does not run a
second session. `state_store:` asserts `{sessions_running_at_end: 1,
attempt_started_by_studio: 2, mac_mini_ended_own_pane_on_return: true}` and
`leases:` asserts `touched_by_studio_before_expiry: false`. This is the run
that proves the single-writer guarantee the whole profile rests on (134, 162,
I15), alongside S17 and S18.

### S22 — the meeting adapter, twice

`satisfies: [111]`. Step 1 is a `direct` step: the operator runs the meeting
adapter by hand on a transcript. A tick reads its signals. The clock advances a
day and the same transcript is imported again.

```bash
flywheel scenario run conformance/scenarios/S22.yaml
```

The adapter is an enumerator (D13): it keys the capture by
`event_key: meeting/2026-09-02/willdan-weekly`, so the second import finds the
capture that exists. The assertions are all negative — `start_session` count 0,
`captures_existing: 1`, `capture_files_written_on_second_import: 0`,
`signal_files_written_on_second_import: 0`. On git-only those last two are
counted commits.

### What a failing expectation prints

The scenario and step, the clause numbers from `satisfies:`, expected against
actual, and the path to the trace:

```
FAIL S13 · step 9 · effects · 147, 150 · I11
  expected  start_session count 1
  actual    start_session count 2  (studio@step 8, mac-mini@step 11)
  trace     target/flywheel-trace/git-only/S13.trace.md
```

## 3. The flags

| flag | meaning |
|---|---|
| `--profile <stand-in\|git-only\|tracker>` | the `StateStore` binding; default stand-in. `git-only` runs against a temporary bare repository, `tracker` needs `FLYWHEEL_SANDBOX_REPO` and is phase 2 |
| `--hosts real` | every host named in the scenario is a real `flywheel host` process; without it hosts are ticked in process |
| `--definitions <dir>` | load the machine files from a directory instead of the embedded set (D2); used by the parity test |
| `--trace` | render each scenario's run as a trace (95) |
| the 390px driver | a headless browser, a test dependency of `flywheel-scenario` and of no shipped crate (D15) |

`cargo test` runs the stand-in path over the whole suite, plus the contract set
against a local bare repository — no network anywhere. By hand: `--hosts real`
runs, the `tracker` profile, and the willdan week.

## 4. How phase 1 uses it

1. **The thirteen `contract/` files, both paths.** One per operation of B.1,
   per guarantee of B.2, plus `binding`, over the toy `lamp` machine, which
   shares no atom with the flywheel — so the profile is tested before any
   domain definition loads. First on the stand-in, then against a local bare
   state repository. This is the step that admits the git-only profile (168)
   and the heart of the gate (Migration Plan step 3).
2. **The twenty-one scenarios on the stand-in.** The phase-1 subset of the
   proposal's acceptance table; the other thirteen are deferred with a reason.
3. **The same twenty-one against git-only**, with S13, S17 and S18 as two real
   host processes and the 390px pass for every scenario carrying a response
   (314, Migration Plan step 9).
4. **The willdan week.** `flywheel host` on the git-only profile with the
   operator as the session binding (93b) and the workspace recorded (93a), on
   real work, with no state edited by hand. That is what ends the phase.
5. **The parity hash.** The run record carries the hash of the machine files it
   ran; a mismatch against `definitions/` fails the run, and `definitions/` is
   itself byte-identical to `models/statechart/` in the blueprints (83, D2,
   task 11.5, 12.4).

## 5. Under-specified, with a proposed answer

**1. How does a step name the host it runs as?** `schema.json` `step.host`
requires only `name` and describes it as "the following steps run as this host
until the next host step"; S13 uses `lose:` and `return:`,
`profiles/sessions-stand-in.yaml` documents `start`, `lose`, `disconnect`,
`return`, and `contract/single-writer.yaml` uses `and:`, `concurrent:` and
`bypass_lease:`, none of which the schema or any prose names.
*Proposed:* close the vocabulary in the schema — `{name}` alone sets the acting
host, and at most one of `start | lose | disconnect | return` performs the
transition. Concurrency moves off the host step to `tick: {concurrent_hosts:
[a, b]}`, and `bypass_lease` becomes a declared contract-only hook the runner
honours only in process. A scenario with no host step runs as a single host
named `local`.

**2. How does time advance?** `schema.json` has `step.clock {advance}`, script
entries carry `after: 2m`, and the prototype advances `now` by a fixed
`tick_seconds` inside `tick()` (`crates/flywheel-scenario/src/runner.rs`, the
end of `Runtime::tick`). Nothing in `specs/scenarios/conformance/spec.md` or
tasks group 11 says which.
*Proposed:* the scenario clock is virtual and moves for exactly two reasons — a
`clock` step, and each `tick` step by one tick interval, declared per run and
defaulting to the 60-second sweep of D7. Real wall-clock time never reaches a
guard. Under `--hosts real` the child hosts take the same virtual clock through
an injected clock source and their sweep is triggered by the runner rather than
by a timer, so a two-host run is deterministic.

**3. How is the scripted exit bound to the same command path?** D8 and
`sessions-stand-in.yaml` both say the stand-in runs "the same `flywheel exit`
command a session would"; task 2.3 says "playing exits through the reporting
command rather than by setting evidence". Neither says subprocess or in-process
call, nor where the session id and the place come from.
*Proposed:* a subprocess of `std::env::current_exe()`, cwd set to the session's
place, session id in the environment under the same name the rendered work
order tells a real session to use. One path for stand-in and `--hosts real`, so
the phase-2 runner changes nothing. Assertions read the thread entry only.

**4. Where does the trace go, and in what format?** `conformance/README.md`
shows `--trace` writing `S30.trace.md`, with no directory; the conformance spec
names the contents; task 11.2 names neither location nor format.
*Proposed:* `--trace [dir]`, default `target/flywheel-trace/<profile>/`, never
beside the scenario files, which are the model's copy. Each run writes
`<scenario>.trace.json` — the structured record the assertions themselves
evaluate — and renders `<scenario>.trace.md` from it, so the document a person
reads and the evidence a failure cites cannot diverge (95).

**5. How is the 390px driver invoked and what does it assert?** D15 says "one
script per scenario carrying an operator's response" and names the three
assertions; tasks 11.7 and 11.8 add "opens the page at a chosen viewport and
taps a control". Which scenarios qualify, which browser, and how the page is
served are all open.
*Proposed:* the runner selects the set itself — every scenario with a
`response` step — so no list is maintained by hand. The runner serves the page
from the process that just ticked, opens `/plan` at 390x844 and again at
1440x900 in a headless Chromium held as a dev-dependency, finds the decision by
the number the register gave it, and asserts D15's three things. No per-scenario
script exists unless a scenario adds an optional selector.

**6. How do `expect` keys map to evidence names?** `then.state_store` is
`{"type": "object"}` in `schema.json` with no properties, and scenarios invent
keys freely: `writes_with_effect_id`, `second_reported_as_write`,
`loser_reread_before_deciding`, `mac_mini_ended_own_pane_on_return`,
`capture_files_written_on_second_import`. An unbound key passes silently today.
*Proposed:* make profile observations a named registry like evidence — each
profile file gains an `observations:` block binding every key it answers, the
schema constrains `state_store` keys to that registry, and `machines/check.py`
fails a scenario using a key some profile it applies to does not bind. A key
`mac_mini_…` naming one scenario's host is renamed to a parameterized form.

**7. What is the exit code and the report format?** Nothing states either —
`conformance/README.md` shows commands only, the conformance spec has no
clause, and tasks 11.1–11.9 say "verify" without saying how a run reports.
*Proposed:* 0 all passed, 1 an assertion failed, 2 a scenario is invalid against
the schema or uses an unbound name, 3 the profile was refused (binding
incomplete, or the definitions hash did not match). One line per scenario, then
a summary; and a run record written through the store holding the profile, the
definitions hash, the scenarios that ran, the subset excluded with the reason
(93a), and the failures (79–82, 167).

**8. How does a scenario declare the profiles it runs on, and what happens when
one is excluded (93a)?** `profiles:` is an enum of store bindings only. The
93a exclusion is a different axis — recorded workspace versus real — and no key
carries it: task 11.4 says "verify the excluded set is S14, S32, X03 and the
others the proposal defers", which is a list held outside the data.
*Proposed:* add `requires: [real-workspace | real-sessions]` to the schema, set
on exactly the scenarios asserting a real take, merge, rebase, conflict or
landing. The runner skips a scenario whose requirements the bound
implementations do not provide, prints it as skipped with the reason, and names
the skipped set in the run record. `profiles:` keeps meaning the store binding
alone. A scenario the phase's acceptance table lists but every configuration
skips is a failure, not a silent pass.

**9. Does the runner run both host processes from one binary?** D15 says
`--hosts real` "starts a second `flywheel host` as its own process"; whether
the first host is also a process, or an engine inside the runner, is unstated,
and `single-writer.yaml` needs two writers racing on one object.
*Proposed:* under `--hosts real` every host in `given.hosts` is a child of
`std::env::current_exe()` with `--root <tmp>/<name>` and a port range from the
router; the runner holds no engine and asserts only through the store. Without
the flag the runner ticks one in-process engine per host name over a shared
stand-in store, which is the only mode where `bypass_lease` is honoured.

**10. How do seeds reference fixture files?** `given.files` is
`{"type": "object", "description": "blueprints or built repository files the
world reports"}` with no path semantics, and S22's `direct` step passes
`flywheel capture meeting 2026-09-02-willdan-weekly.vtt` — a path with no base
and no such file anywhere under `conformance/`.
*Proposed:* add `conformance/fixtures/` and resolve every path in
`given.files`, a `files` step and a `direct` argument relative to it; short
files may be given inline as path-to-content and are materialized into the
scenario's temporary checkout. S22's transcript becomes
`conformance/fixtures/meeting/2026-09-02-willdan-weekly.vtt`.

**11. What is the shape of a `direct` step?** `schema.json` gives it
`{"type": "object"}` with a prose description covering four unlike things — a
commit, a board move, an issue close, an adapter invocation — and S22 uses an
`adapter:` key that appears nowhere else.
*Proposed:* one required discriminator, `direct: {do: adapter | commit |
board | close, …}`, each arm with its own required fields, so a typo fails the
schema rather than being ignored.

**12. How does `response.decision` resolve to a number?** The schema allows
`decision` (an id) or `number`, and S01 uses the id form; the prototype's
`drive` reads `number` or `object` only and silently ignores a step carrying
`decision` or `id` (`crates/flywheel-scenario/src/scenario.rs`, the `response`
arm).
*Proposed:* the runner resolves `decision` through the register at the moment
the step runs and fails the scenario when it names no standing decision, so the
id form is the readable default and the `number` form stays available for the
"answer it again by number" assertions (15).
