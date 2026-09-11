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

Two runs, and the difference is what a test costs, never what it proves.

| when | command | costs |
|---|---|---|
| while iterating | `cargo test -p <the crate you touched>`, or `cargo test --workspace` | 137 s of test time |
| at a group's end, and at 12.4 | `cargo test --workspace -- --include-ignored` | 513 s |

Every test runs over a real state repository: a bare repository on this
computer and a checkout of it, which is the only store this release binds (92).
A tick spends at most one `git push` and nothing else: the fetch, the reads and
the writing of blobs, trees, commits and refs all happen in process through
`gix`, so a test is not paying for a process per read and not even for the fetch
(`git-only.yaml` Tools, 169). `conformance/contract/cost.yaml` is what holds
that: it asserts the counts the store keeps of itself —
`subprocesses_per_tick: [0, 1, 0]`.

Marked `#[ignore = "group gate: …"]` and left out of the default run are only
the tests that **start a process of their own**: a host under `--hosts real`, a
headless browser for the 390px pass, or the `flywheel` binary itself. They are
not optional — a group is not done until they pass, and 12.4 runs them.

Where the time goes, so a change that costs something is noticed:

| binary | default | with the ignored |
|---|---|---|
| `flywheel-scenario/tests/cascade.rs` | 37 s | 39 s |
| `flywheel/tests/host.rs` | 23 s | 20 s |
| `flywheel/tests/effects.rs` | 15 s | 15 s |
| `flywheel/tests/curate.rs` | 12 s | 12 s |
| `flywheel-scenario/tests/conformance_runner.rs` | 8 s | 23 s |
| `flywheel/tests/init.rs` | 7 s | 7 s |
| `flywheel-store-git/tests/disconnected.rs` | 7 s | 7 s |
| `flywheel-surface/tests/dictation.rs` | 7 s | 7 s |
| `flywheel/tests/signals.rs` | 6 s | 6 s |
| `flywheel/tests/walkthrough.rs` | — | 165 s |
| `flywheel-scenario/tests/phone.rs` | — | 143 s |
| `flywheel-scenario/tests/real_hosts.rs` | — | 48 s |
| everything else | under 4 s each | |

`walkthrough.rs` is the largest of the ignored: it starts a real host with
`--serve` and plays README.md's whole turn of the loop against it, and the
machinery's own cascade advances about one transition per pass while the loop's
quiet interval is the 30-second poll (`host::POLL`, D6, D7). A local cause — a
page response, a session's report — wakes that loop at once; nothing yet
shortens the passes the cascade itself needs.

A test that reaches a real process is marked when it is written, so the default
run stays the one a person runs every few minutes. A test never sleeps: the
clock is virtual and moves for a `clock` step and a tick interval alone (D15).

## Vocabulary

Use the requirements' words exactly: instance, account, host, place, capture,
signal, decision, response, intent, elaboration, bolt, unit, stage, proposal,
claim, sink, tick, tier, rail. Not job, task, ticket, pin, mint. An instance is
what the state holds and an account holds one or more instances; organization is
for a company or the git host's organization alone.
