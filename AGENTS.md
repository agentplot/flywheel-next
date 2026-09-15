# Working in flywheel-next

This repository is the **flywheel binary**: the open-source product. Everything a
self-managed operator runs lives here, and nothing else does.

## Where the design lives

The requirements and the model are in the blueprints repository, not here:

- `agentplot/blueprints` → `design/flywheel-next/requirements.md` (numbered
  clauses; cite them as `(204)`), `surfaces.md` (S-numbered surface rulings),
  `roadmap.md` (the five phases), `models/statechart/` (the machines and
  profiles this repo's `definitions/` mirror), `proposals/` (narrative behind
  the later sections), `generic/` (business processes on the flywheel).
- Local checkout: `/Users/chuck/Code/github_agentplot/blueprints/main/design/flywheel-next/`.

`definitions/` is a byte-for-byte mirror of the model's machines and profiles,
`conformance/` of the model's conformance suite (scenarios, contract,
fixtures, observations, schema), and `instructions/` of the model's shipped
instruction set (the default instructions, the schema per deliverable and per
unit-type stage, the producer skills, and a skill and definition per agent;
`set.yaml` attaches the defaults by type tier). Change the model there first,
then copy; never edit `definitions/`, `conformance/` or `instructions/` by
hand.
`check.py` in the model must report every clause cited before a definition
lands here.

## The two products

| product | repository | holds |
|---|---|---|
| the binary (this repo, open source) | `agentplot/flywheel-next` | machines and profiles, the page bundle (rail console and management console), the tool server and MCP endpoint, adapters, runners, routers, the definitions of permissions, roles, features, flags and plans, the CLI, the templates for the blueprints and built repositories |
| the control plane (commercial) | `agentplot/flywheel-cloud` (private) | the receiver, dispatcher packaging and its roles and tags, queues, scheduler, warm cache and page projection stores and keys, page distribution, registry, deployer, identity environment sync, plans and billing, pool image build and provisioning, the shared chat applications |

The line between them is the **invocation contract** (requirements A.37,
296–305): the control plane invokes this binary in two modes, tick and request,
with a stated environment. That contract is documented in this repository and is
public. Nothing that assumes a multi-tenant service, an AWS account of ours, a
payment provider or a Frontegg environment of ours belongs here; it belongs in
flywheel-cloud. A self-managed host and a hosted host run the same bytes from
this repository.

## How work is done here

- Work is OpenSpec changes under `openspec/changes/`, one per phase of the
  roadmap, named for the phase (`the-loop` is phase 1; then construction, context, dispatch, scale). Artifacts are written in order:
  proposal, design, specs, tasks; each artifact is reviewed before the next is
  started; `apply` runs only after the review of the tasks.
- Every claim in a proposal or design cites the requirement clause it satisfies.
  A design that needs a clause the requirements do not have proposes the clause
  to the blueprints repository rather than inventing it here.
- Rust, workspace crates: `flywheel-engine` (definitions, guards, tick,
  decisions, the rec format; no domain name in it), `flywheel-scenario` (the
  store and world, scenario seeding), `flywheel-surface` (the page and API),
  `flywheel` (the binary). Keep that split; add a crate rather than widening one.
- `cargo test` is the gate. A scenario under `scenarios/` is the acceptance for
  a change and mirrors a scenario in the model's `conformance/`. See **Gates**.
- Conventional Commits. Never commit `state/` or `target/`.
- Secrets never appear in configuration or code; a host reads them from the
  place the operator put them (requirements 204, 207).

## Gates

Three tiers, and a test's tier is decided by its **subject** — what the test is
about. What it starts is a symptom and not the rule. A test in the wrong tier is
a defect like any other (D17).

| tier | subject | lives in | run by | the bar |
|---|---|---|---|---|
| unit | one crate's own code; for the store crates a temp repository is part of that subject rather than a dependency of it | `src/**` in `#[cfg(test)] mod tests`, beside the code | `cargo test --lib` | the whole tier under 10 s; high coverage, and a new public function arrives with its tests |
| integration | a seam between crates, over a real repository, in process | `crates/<crate>/tests/*.rs` | `cargo test --workspace` | happy paths only, as few as cover the seams; no single test over 5 s |
| system | the product as a whole — against the model, or against a real host, a real binary, a browser | `crates/<crate>/tests/system/main.rs`, one target per crate | `cargo test --workspace --features system-tests` | it costs what it costs, and it runs at merge |

The acceptance and contract sets start no process and run in this one, and they
are still system tier: what they take as their subject is the whole binary
against the model's conformance suite (168, D15).

**The fake store.** `flywheel-domain` and the projections take the `StateStore`
trait, so a fake that holds objects in a map is all the first tier needs, and a
fake is preferred to a mock: it behaves, it is not told what to expect. It is
`flywheel_atoms::testing::FakeStore`, behind a `testing` feature, a
dev-dependency of the crates that use it, and it is never named by `--profile`,
never bound in a `profiles/` file and never present in the acceptance set. That
is what keeps it clear of 92: what 92 retired was a second store *profile*
claiming conformance, and this claims nothing. `flywheel_surface::testing`
holds the fake `World` beside it.

**The git store's own tests are unit tests.** A temp bare repository and a
checkout are the store's subject, not a dependency of something else, and a
tick against them spends at most one process, so they live in
`flywheel-store-git/src/tests/` and hold to the first tier's bar.

**Tier three is a feature, not an `#[ignore]`.** `[[test]] required-features =
["system-tests"]` keeps those targets out of the default build entirely, so the
everyday run saves their compile time as well as their wall time. Nothing is
marked `#[ignore]` to hide cost.

**The toolchain is devenv's.** `devenv.nix` beside this file pins a native
rustc and cargo for this computer, and every cargo command here, in README.md
and in `.config/wt.toml` runs inside `devenv shell`: `devenv shell -- unit`,
`devenv shell -- integration`, `devenv shell -- system` are the three tiers by
name. A toolchain the machine happens to have is not used, because one that
was the wrong architecture once made the two-minute tier take half an hour,
and nothing in the suite could show why.

**Worktrunk runs the tiers at the right moment.** `wt hook pre-commit` runs the
unit tier and `wt hook pre-merge` runs the integration tier, so a branch cannot
land without the seams proven and nobody waits on them while working. The
system tier is run by a person, when the brief says so.

**What you run, and where a new test goes.** While building, the one crate you
touched: `cargo test --lib -p <crate>` continuously. Before a commit,
`devenv shell -- unit` — the whole first tier, seconds. That is all a session
runs on its own: the integration tier is the pre-merge hook's and the system
tier is not yours except when the brief says so. A new behaviour arrives with
unit tests for the logic beside the code, over `flywheel_atoms::testing`'s fake
store where a store is needed; an integration test is added only where the
seam between crates is itself the thing under test, and then only on the happy
path, because every one of them is paid for on every merge. What proves a page
is a screenshot read by a person, not a test that finds an id.

Every test that touches a store runs over a real state repository: a bare
repository on this computer and a checkout of it, which is the only store this
release binds (92). A tick spends at most one `git push` and nothing else: the
fetch, the reads and the writing of blobs, trees, commits and refs all happen in
process through `gix`, and a tick's commits go at the shared line in that one
push, so a test is not paying for a process per write, per read, or even for the
fetch (`git-only.yaml` Tools, 167, 169).
`conformance/contract/cost.yaml` is what holds that: it asserts the counts the
store keeps of itself — `subprocesses_per_tick: [0, 1, 0]`.

A test never sleeps: the clock is virtual and moves for a `clock` step and a
tick interval alone (D15). Where a test must drive the running loop it sets
`flywheel.yaml`'s `intervals.poll` and `intervals.sweep` to milliseconds: those
are the backstop for what nothing notified, and a test is the thing doing the
notifying (D6, D7, 130, 231).

Where the time goes, measured on the native toolchain on 2026-09-14 with the
build warm, so a change that costs something is noticed. The system tier's
acceptance driver gates on the twenty-one rows phase 1 lists; the rows a later
phase takes up that fail (S9, S10, S12, S25, S31, X2, X4, X7, X10) or do not
validate (S3, S27, S28, S30) are in its report and fail nothing.

| tier | tests | wall time |
|---|---|---|
| `devenv shell -- unit` | 195 | 13 s |
| `devenv shell -- integration` | 294 | 178 s |
| `devenv shell -- system` | 202 | 434 s |

| binary | tests | time |
|---|---|---|
| `flywheel-scenario/tests/cascade.rs` | 5 | 34 s |
| `flywheel/tests/apply.rs` | 6 | 29 s |
| `flywheel/tests/host.rs` | 26 | 22 s |
| `flywheel-scenario/tests/conformance_runner.rs` | 13 | 14 s |
| `flywheel/tests/effects.rs` | 5 | 12 s |
| `flywheel/tests/served.rs` | 6 | 12 s |
| `flywheel/tests/init.rs` | 12 | 11 s |
| `flywheel/tests/curate.rs` | 1 | 9 s |
| `tests/atoms.rs` | 1 | 5 s |
| `tests/catalogue.rs` | 1 | 5 s |
| everything else | | under 5 s each |

Almost all of it is `git push`: the push is the compare-and-swap the whole
profile rests on (134, 162), it costs about 50 ms to a bare repository on this
computer, and a test that proves a race pays for one. So a test pays for the
pushes its proof needs and never for a repetition of them: proving a renewal
touches no commit on `main` takes three renewals, not a hundred, because nothing
in a renewal is quantity-dependent.

`cargo nextest` was measured and is not adopted: it runs every test in a process
of its own, and with a bare repository and a checkout made per test that is four
times slower here — 528 s against cargo's 127 s.

## Vocabulary

Use the requirements' words exactly: instance, account, host, place, capture,
signal, decision, response, intent, elaboration, bolt, unit, stage, proposal,
claim, sink, tick, tier, rail. Not job, task, ticket, pin, mint. An instance is
what the state holds and an account holds one or more instances; organization is
for a company or the git host's organization alone.
