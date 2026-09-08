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

`definitions/` is a byte-for-byte mirror of the model's machines and profiles.
Change the model there first, then copy; never edit `definitions/` by hand.
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
  a change and mirrors a scenario in the model's `conformance/`.
- Conventional Commits. Never commit `state/` or `target/`.
- Secrets never appear in configuration or code; a host reads them from the
  place the operator put them (requirements 204, 207).

## Vocabulary

Use the requirements' words exactly: organization, host, place, capture,
signal, decision, response, intent, elaboration, bolt, unit, stage, proposal,
claim, sink, tick, tier, rail. Not job, task, ticket, pin, mint, instance.
