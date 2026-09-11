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

**Curate, on the page.** Curation reads the signals with no move, and the tick
charges it when the cadence says so or when twelve are waiting (110). The
shipped threshold is twelve, so capture a dozen things — or wait for the
cadence, which is weekday mornings. When curation is charged, the status view
shows what it raised under **run by the operator** — the operator is the session
in this phase (93b):

```
curation/willdan   run: running   held by laptop (alive)   run by the operator
```

and the board's Inception lane gains the curator's surface: every unmoved
signal, quoted, with the standing moves beside it as controls — attach, join,
route, challenge, drop — and a field for what each one names (107, 116, 194).
Nothing there reads a word of the signal for meaning; the move is a control and
the target is picked.

Give two of them `join` and name the same intent — `intent/rows-lose-numbers`,
say; it need not exist yet — and submit. That one submit is the curation
session's whole delivery and its exit: it writes one move record per signal and
reports `done` with `move` as what it delivered, which is the record
`flywheel exit done --deliverable move` writes from a place (67, 93b). The same
report from a terminal is:

```sh
FLYWHEEL_SESSION=curation/willdan/main/1 \
FLYWHEEL_STATE=~/flywheel/hosts/laptop/willdan/flywheel-state \
  cargo run -q -- exit done --deliverable move --host laptop
```

**The decision, and the answer.** On the next tick the machinery applies what
the session delivered: each judged signal gets its one standing move, and the
joins become a proposed intent citing them, which is its weight (107, 109, 110,
116). A proposed intent is one decision on the rail, with a number the register
gave it:

```
approve
  3   intent/rows-lose-numbers        2 signals from page
      [ yes ]  [ no ]  [ later ]
```

Tap `yes` — on the laptop or on the phone, the same control either way. It posts
to the same tool a numbered chat reply calls, the answer is recorded at once with
who gave it and when, and a reload shows it there before the next tick applies
it (15, 153, 154, 193, 311).

**Read it all back.** Every write is a commit, and the record is readable with
nothing running (160, 167):

```sh
git -C ~/flywheel/git-host/willdan-state.git log --oneline
git -C ~/flywheel/git-host/willdan-state.git show --stat HEAD
```

The captures, the signals, the moves you made on the page, the proposed intent
they produced, the answer you gave it and the status projection are each a
commit, with the reason and the evidence the guard read in the message (79,
127). Nothing in that record was written by hand.

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
