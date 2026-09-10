# Conformance suite

One set of scenarios, run as data, that every profile must pass
unchanged (requirements section 12, 168). A profile is admitted when
`flywheel scenario run --profile <name> conformance/` passes with the
machine files byte-identical to `machines/` (their hash is written into
the run record).

Two directories, plus two files that bind the vocabulary:

- `contract/` — one scenario per operation of B.1, per guarantee of
  B.2, and one for the binding itself, exercised through the engine
  with a toy machine that shares no atom with the flywheel (`lamp`), so
  a profile is tested before the domain is loaded.
- `scenarios/` — S1 to S34 of the requirements, run over the flywheel's
  own machines; X1 to X9 for the requirements the numbered scenarios
  do not reach; T1 for the tracker's direct action. The `profiles:`
  line of each says which state stores it applies to; `all` runs on
  every state-store profile there is — git-only in the first build,
  the tracker beside it in the second.
- `schema.json` — the scenario schema. Every vocabulary in it is
  closed, so a misspelled key fails validation rather than being
  ignored in silence.
- `observations.yaml` — the observation registry: every key a scenario
  may assert under `then.state_store`, what it means, and which
  profiles answer it.
- `fixtures/` — the world's inputs on disk. Every path a scenario names
  resolves here.

Every scenario states the requirements it exercises in `satisfies:`;
`machines/check.py` fails when a number names no requirement, a
requirement is cited by nothing, an observation key is not bound, or a
scenario outside `contract/` declares a hook.

## The format

```yaml
scenario: S1
title: approve a proposed elaboration from the phone
profiles: [all]            # the state store binding: all, git-only, tracker
requires: [real-workspace] # optional; see "What a run must provide"
satisfies: [1, 6, 13, 24]
invariants: [I1, I2]
given:                     # the described state of the stores
  objects:
    - {id: intent/atlas-provider-limits, machine: intent, state: {life: open, line: current, open.material: settled, open.close: not-offered}}
    - {id: elaboration/atlas-provider-limits/research-1, machine: elaboration, parent: intent/atlas-provider-limits,
       state: {life: proposed}, record: {type: self-closing, type_version: 1}}
  evidence:                # evidence values the run reports; anything unlisted is absent/false/none
    session.pane: {"elaboration/atlas-provider-limits/research-1/self-closing/1": absent}
  register: {elaboration/atlas-provider-limits/research-1/elaboration-proposed: 7, next: 8}
  marks: {chat: now}
  script:                  # what the stand-in sessions play (93)
    "elaboration/atlas-provider-limits/research-1/self-closing/1":
      - {after: "0", pane: present, activity: working}
      - {after: "2", exit: done, deliverables: [note], commits: [research.md]}
when:                      # steps in order
  - tick: {}
  - response: {decision: elaboration/atlas-provider-limits/research-1/elaboration-proposed, answer: "yes", id: discord/1001}
  - tick: {}
  - clock: {advance: 40m}
  - evidence: {place.exists: {"…": true}, place.contains_line: {"…": true}}
  - tick: {}
  - response: {number: 7, answer: "yes", id: discord/1001}    # the same delivery again, by number
  - tick: {}
then:
  transitions:             # in order, across all ticks
    - {object: elaboration/atlas-provider-limits/research-1, from: proposed, to: approved, response: discord/1001}
    - {object: …, from: approved, to: placing}
  effects:                 # by atom name, with count; a repeat must not add
    - {do: prepare_place, count: 1}
    - {do: start_session, count: 1}
  decisions:
    - {after_step: 1, present: [], absent: [elaboration-proposed], numbers: {…: 7}}
  writes: {max_per_tick_when_unchanged: 0}
  applied_responses: {elaboration/atlas-provider-limits/research-1: [discord/1001]}
```

Every `then` clause is asserted; an unlisted effect with count 0 is
asserted absent when `effects_closed: true`.

## The steps

| step | what it does |
|---|---|
| `tick: {}` | one tick over the scope, as the acting host |
| `tick: {concurrent_hosts: [a, b]}` | those hosts tick at the same moment against the same store, which is how a race is written |
| `tick: {interval: 5m}` | this tick moves the clock by that much instead of the run's interval |
| `tick: {run: <path>}` | run the named scenario file to completion in place of ticking this one (95) |
| `response: {decision, answer, id, by}` | a delivery through `receive`, naming the decision by id |
| `response: {number, answer, id}` | the same, naming the number the register gave (15) |
| `response: {object, answer, id}` | a dictation naming the object it acts on |
| `evidence: {…}` | the world changed: set the evidence the run reports |
| `script: {…}` | seed or extend what the stand-in sessions play |
| `notify: {…}` | a notify for an object |
| `restart: {}` | drop every in-memory thing and start again |
| `disconnect: {}` / `reconnect: {}` | the state store refuses, then resumes (git-only, tracker) |
| `host: {name}` | the following steps run as this host |
| `clock: {advance, at}` | move the virtual clock |
| `direct: {do, …}` | something acted outside the machinery's loop |
| `files: {…}` | what the world reports on disk |

### The acting host

`host: {name}` sets which host the steps after it run as, until the
next host step. `name: none` means no host is acting; a scenario with
no host step at all runs as a single host named `local`.

Beside `name`, a host step carries **at most one** transition, and the
set is closed: `start` (a host that was not running joins fresh),
`lose` (lost without warning, no shutdown and no release), `disconnect`
(still running, cannot reach the store), `return` (a lost or
disconnected host comes back). Anything else fails validation.

Concurrency is not a property of the acting host, so it is not written
there: two hosts racing is `tick: {concurrent_hosts: [a, b]}`.

### The clock

The scenario clock is virtual and moves for exactly two reasons: a
`clock` step, and each `tick` step by one tick interval, declared per
run and defaulting to the profile's sweep. Real wall-clock time never
reaches a guard, so a run is deterministic and a scenario that waits
says so in its own steps. `clock: {advance: 6m}` moves it; `at:` pins
the time of day it lands on, for a cadence a scenario must hit exactly.

A script entry's `after:` is a **step number**, never a duration: it
says which step the entry plays at. A session that should act after
time passes is written as a `clock` step in `when` followed by the tick
that reads it.

### Direct steps

`direct` is something acting outside the machinery's own loop, which
the machinery must then read as it finds it. `do` names which, and each
arm carries its own fields:

| `do` | fields | what it is |
|---|---|---|
| `dictation` | `text`, `by` | the operator said this in chat; the host's agent proposes the call and the confirmation is the response (194) |
| `adapter` | `command`, `by` | an adapter's own binary writing a capture from any machine (217g) |
| `commit` | `file`, `set`, `sha`, `by` | the operator edited the state where it is kept and committed (3, 159) |
| `board` | `issue`, `column`, `event_id`, `by` | the operator moved a card on the tracker's board (159) |
| `close` | `issue`, `event_id`, `by` | the operator closed the issue behind an object |
| `page` | `path`, `by` | the operator opened a page route |
| `shell` | `command`, `by` | the operator ran something by hand, outside every tool (S15, X08) |
| `projection` | `object`, `set` | a projection drifted from the state it projects (136) |
| `store` | `object`, `set` | something else changed the object in the store (130) |

### `response.decision`

The schema keeps the id form, `<object id>/<decision kind>`, because it
is the readable one. The runner resolves it **through the register at
the moment the step runs**, and fails the scenario when it names no
standing decision, so a scenario cannot quietly answer nothing. The
`number` form stays available for the assertions about answering the
same decision again by its number (15).

## Fixtures

Every path in `given.files`, in a `files` step, and inside a `direct`
command resolves against `fixtures/`. A path naming a file there is
materialized into the scenario's temporary checkout before the run. A
`files` value that is not a fixture path is taken as inline content for
the path it is keyed by, so a one-line file needs no file of its own. A
path resolving to neither is invalid, and the run exits 2.

`fixtures/README.md` lists what is there and which scenario reads it.

## Observations

`then.state_store` is where a scenario asserts something about the
profile's own behaviour rather than about a machine: a rejection, an
effect id, an as-of point, what a host did while it could not reach the
store. Those are not evidence names, so nothing used to bind them and
an invented key passed in silence.

`observations.yaml` binds them. Each key names what it means and which
profiles answer it, and `machines/check.py` fails a scenario using a
key the registry does not hold, or a key some profile the scenario runs
on does not answer. It also fails a bound key no scenario asserts, so
the registry cannot grow stale.

A key is host-agnostic. An observation about one host is keyed by host
in its value, `{mac-mini: true}`, never by a host's name in the key.

## What a run must provide

`profiles:` names the state store binding and nothing else. Whether the
workspace and the sessions are real is a second axis, and `requires:`
carries it (93a):

- `real-workspace` — the scenario asserts a real take, merge, rebase,
  conflict or landing, or a real place, hook or tethered process. The
  recorded workspace stand-in writes each proof's evidence and touches
  no repository, so it cannot serve these.
- `real-sessions` — the scenario asserts a real agent session rather
  than a scripted one.

A run whose bound implementations do not provide a requirement skips
the scenario, prints it as skipped with the reason, and names the
skipped set in the run record. A scenario the phase's acceptance table
lists but every configuration skips is a failure, never a silent pass.

## Hooks

`hooks:` declares in-process fault injection the runner honours only
when it holds the engine itself, which is the mode without
`--hosts real`. Two exist: `bypass_lease`, where the acting hosts
attempt their writes without taking the lease first, and
`interrupt_write`, where the acting host's write is cut off midway.

Both force a race the machinery is built to prevent, so they belong to
the proofs of B.2 and nowhere else. `check.py` fails a scenario outside
`contract/` that declares one.

## The stand-in sessions

`profiles/sessions-stand-in.yaml` binds the session evidence and
effects to a scripted player. `start_session` records the id and plays
the script entries for it at their steps: a pane appearing, activity, a
keystroke, an exit reported by running the same `flywheel exit` command
a session would, offers by `flywheel offer`, a refusal by
`flywheel refuse`, commits made into the place with git. Only this
binding is faked (93); the state store, the engine, the git effects on
real (sandbox) repositories, the rail and the page run as built. A
scenario may set the session evidence names directly in an `evidence`
step instead; the script is the way to say what a session would have
done.

## Running

```bash
flywheel scenario run conformance/                      # the no-live-service run (92): git-only against a temporary bare repository, stand-in sessions, every file
flywheel scenario run --profile git-only conformance/   # the same, named
flywheel scenario run --profile tracker  conformance/   # against a throwaway GitHub repository (needs FLYWHEEL_SANDBOX_REPO)
flywheel scenario run conformance/scenarios/S30.yaml --trace
```

There is no store of the runner's own. `flywheel-scenario` stands the
bare repository up, binds `git-only.yaml` over it exactly as a host
would, plays the scripted `Sessions` stand-in, and runs a `World`
whose git and wt calls act on sandbox repositories; the tracker
profile runs the same files with a throwaway GitHub repository as its
store.

### The trace

`--trace [dir]` writes to `target/flywheel-trace/<profile>/`, never
beside the scenario files, which are the model's copy. Each scenario
writes `<scenario>.trace.json`, the structured record the assertions
themselves evaluate, and `<scenario>.trace.md` rendered from it, so the
document a person reads and the evidence a failure cites cannot diverge
(95). The trace lists the ticks, the guards evaluated, the transitions
taken, the effects performed and the decisions created and retracted.

### Exit codes

| code | meaning |
|---|---|
| 0 | every scenario passed, and every scenario skipped was skipped for a stated requirement |
| 1 | an assertion failed |
| 2 | a scenario is invalid against the schema, or uses a name nothing binds: an unknown evidence or effect name, an unbound observation key, a fixture path resolving to nothing |
| 3 | the profile was refused: its binding is incomplete (140), or the definitions hash did not match the repository's |

The run prints one line per scenario and then a summary.

### The run record

A run writes a record through the store holding the profile, the hash
of the definitions it ran against, the scenarios that ran, the subset
skipped with the requirement each was skipped for (93a), and the
failures (79–82, 167).
