# flywheel next

The flywheel binary: the engine that reconciles declared state machines, the
state store over a git repository, the host loop, the page, and the conformance
runner. `definitions/` mirrors the statechart model in the blueprints repository
(`design/flywheel-next/models/statechart/machines` and `profiles`);
`conformance/` mirrors its suite; the requirements are
`design/flywheel-next/requirements.md` there.

One state store is bound in this release: the **git-only** profile — a bare
repository on a computer you control, no live service behind it (92). The
sessions are the operator's (93b) and the lines and places are recorded (93a);
everything else — the engine, the register, the rail, the effects, the leases,
the run record — is real.

## Run the loop on your laptop

No GitHub, no App key, no network: the git host is a directory of bare
repositories on this computer, and every command below is copy-pasteable from
the repository root.

```sh
# Where the instance lives: a directory the repositories go in, and a manifest
# of your own.
mkdir -p ~/flywheel/git-host

# The connection. The App is how the machinery reaches a git host it does not
# own (207); a directory on this computer needs no credential, and what init
# waits for is that you have placed one, never that an agent made it true
# (207a). Placing it is this line.
export FLYWHEEL_APP_KEY=local

# The instance and its first host. `--address` is what this computer is called
# on your own network: a host has one address and it is never a localhost port,
# because every link a delivery carries is written at it (205a, D10a). Every
# step proves itself, so running this again changes nothing and a half-finished
# bootstrap finishes here (204).
cargo run -q -- init \
  --instance willdan --host laptop \
  --root ~/flywheel/hosts/laptop --git-host ~/flywheel/git-host \
  --address http://your-laptop.local \
  --manifest ~/flywheel/flywheel.yaml

# What this host clones under its root, and whether the layout is what the
# manifest says. A host will not start on a layout it did not make (205, 222).
cargo run -q -- host join   --name laptop --manifest ~/flywheel/flywheel.yaml
cargo run -q -- host doctor --name laptop --manifest ~/flywheel/flywheel.yaml --layout

# The loop, with the page beside it: one process over one store (D7, D11).
cargo run -q -- host --name laptop --manifest ~/flywheel/flywheel.yaml \
  --serve 4242 --operator chuck
```

The host says what it bound:

```
host laptop · instance willdan · world host · workspace recorded · sessions operator
page at http://127.0.0.1:4242/
```

**At this machine.** Open <http://localhost:4242/>. The page is one bundle,
rendered from the state on every request: the decisions, the capture box, the
board and the status view (307, D11).

**On your phone, on the same network.** The host binds its private-network name
and the localhost port, and nothing else (46, 155, 245). Where the name you
gave `--address` is one this computer answers at, the host binds it too and the
page is at `http://your-laptop.local:4242/` from the phone; where it is not, the
host says so and serves the localhost port alone. A request to any other
address is refused and says why (253a).

**One turn of the loop.** In the box at the top of the page, type what you
noticed and submit it. Nothing in the text is parsed — no `intent:` or
`bolt <name>:` prefix means anything — and the submission is recorded once as a
capture with one signal of kind ask (19, 111, 193, 194). The status view shows
it straight away:

```
unmoved signals   12 from page, oldest 0d
```

Curation reads the signals with no move, and it is charged when the cadence says
so or when twelve are waiting (110). When it is, the status view shows what it
raised under **run by the operator** — the operator is the session in this phase
(93b) — with the command to report by:

```
curation/willdan   run: running   held by laptop (alive)   run by the operator
```

The work order names the session and this host's checkout of the state
repository, and a report is one command from the place it runs in (67, 89):

```sh
FLYWHEEL_SESSION=curation/willdan/main/1 \
FLYWHEEL_STATE=~/flywheel/hosts/laptop/willdan/flywheel-state \
  cargo run -q -- exit done --deliverable "twelve signals judged" --host laptop
```

What curation delivers is one move per signal and the proposed intents the joins
produce; each proposed intent is one decision on the rail, answerable on the
page or from the phone, and the answer is the same tool a numbered chat reply
calls (107, 109, 15, 193). In this release those move records are yours to
write — a person writing them by hand is curation too, and the machine then
finds nothing left to do (`curation.yaml`).

**Read it all back.** Every write is a commit, and the record is readable with
nothing running (160, 167):

```sh
git -C ~/flywheel/git-host/willdan-state.git log --oneline
git -C ~/flywheel/git-host/willdan-state.git show --stat HEAD
```

The captures, the signals, the responses you gave on the page and the status
projection are each a commit, with the reason and the evidence the guard read in
the message (79, 127).

## The conformance suite

```sh
cargo run -q -- scenario run conformance/            # the whole phase-1 set
cargo run -q -- scenario run conformance/scenarios/S16.yaml --trace
```

`--profile` names the state store binding; `git-only` is this release's only one
and its default, and the trace goes to `target/flywheel-trace/git-only/` (92,
D15).

## What a session reports

The operator is the session in this phase (93b). A work order names the session
and this host's checkout of the state repository, and a report is one command
from the place it runs in (65, 67, 89):

```sh
flywheel exit done --deliverable research.md
flywheel offer chore --document chores/1.md
flywheel note "what it took"
flywheel refuse "this is not the work"
```

`FLYWHEEL_SESSION` and `FLYWHEEL_STATE` are what the work order sets; `--session`
and `--state` say the same thing by hand.

## The crates

| crate | holds |
|---|---|
| `flywheel-engine` | the loader, the guard algebra, the tick, decision derivation and the register; no domain name in it |
| `flywheel-atoms` | the evidence and effect registries, the four traits, the scenario file types |
| `flywheel-domain` | the shipped machines, the object envelope and the record schemas, the work order |
| `flywheel-store-git` | `StateStore` over the state repository |
| `flywheel-world-host` | `World` over git, the manifest and the host's router |
| `flywheel-workspace-recorded` | `Workspace` as records (93a) |
| `flywheel-sessions-operator` | `Sessions` with the operator as the session (93b) |
| `flywheel-surface` | the page, the chat sink, the tool catalogue and its HTTP server |
| `flywheel-scenario` | the conformance runner, the scripted sessions, the trace renderer |
| `flywheel` | the binary: `init`, `host`, `scenario`, `capture`, `exit`, `offer`, `note`, `refuse` |

## Working here

Read `AGENTS.md` first: where the design lives, what belongs in this repository,
and how work is done.
