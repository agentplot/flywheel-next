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
repositories on this computer, a session is Claude Code in a pane of the Herdr
this shell runs in, and every command below is copy-pasteable from the
repository root.

```sh
# Where the instance lives: a git host, the mirror of this repository the
# instance tracks, and a manifest of your own. The mirror is named
# <instance>-<repository>; a repository already there is adopted, never remade
# (204, 205).
mkdir -p ~/flywheel/git-host
git clone --bare . ~/flywheel/git-host/agentplot-flywheel-next.git

# The connection. The App is how the machinery reaches a git host it does not
# own (207); a directory on this computer needs no credential, and what init
# waits for is that you have placed one (207a). Placing it is this line.
export FLYWHEEL_APP_KEY=local

# The instance, the repository it tracks, and its first host. A place is a
# worktree of that repository (`--workspace host`) and a session is an agent in
# a pane of Herdr (`--sessions herdr`). The host's address is this computer's
# own name unless `--address` says otherwise, and never a localhost port,
# because every link a delivery carries is written at it (205a, D10a). Every
# step proves itself, so running this again changes nothing (204).
cargo run -q -- init \
  --instance agentplot --host laptop \
  --root ~/flywheel/hosts/laptop --git-host ~/flywheel/git-host \
  --repository flywheel-next --workspace host --sessions herdr \
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
host laptop · instance agentplot · world host · workspace host · sessions herdr
page at http://127.0.0.1:4242/
```

**At this machine.** Open <http://localhost:4242/>. The page is one bundle,
rendered from the state on every request: the rail of decisions, the capture
box, the four lanes of the board and the dock (307, D11).

**On your phone, on the same network.** The host binds its private-network name
and the localhost port, and nothing else (46, 155, 245). Where the name is one
this computer answers at, the page is at `http://<your-computer>.local:4242/`
from the phone; where it is not, the host says so and serves the localhost port
alone (253a).

**A capture.** In the box at the top of the page, type what you noticed —
`README's crates table lacks a row for flywheel-workspace-host`, say — and
submit it. Nothing in the text is parsed; the submission is recorded once as a
capture with one signal of kind ask, and it stands in the Inception lane,
quoted, with one control beside it: `unit…` (19, 111, 193, 194).

**A unit.** Tap `unit…`. The dock opens on the capture with a field for the
bolt's name; give it one — `readme-crates-table` — and submit. That is a
dictation, applied at once: the unit stands approved on a bolt of that name,
made if it was not there; the bolt's line is a branch of `flywheel-next` on the
git host; the unit's one work item has a place, a worktree of that line under
the host's root; and the Construction lane shows the bolt with its unit and
the session under it (34, 42, 44, 12).

**The session.** On the next tick the item enters its `fix` stage and the host
starts the session: a Herdr workspace named for the bolt, a tab named for the
unit, and Claude Code in the tab's pane at the place, told to read
`.flywheel/work-order.md` there. The order carries the capture's words as the
job, what to deliver, the rules, the chore-fixer's skill, and the exact line to
report with (67, 89, 196). Watch it work in the pane. When it is done it
reports with that line, which from the place is:

```sh
FLYWHEEL_SESSION=work-item/flywheel-next/readme-crates-table/wi-1/fix/1 \
FLYWHEEL_STATE=~/flywheel/hosts/laptop/agentplot/flywheel-state \
  cargo run -q -- exit done --deliverable commits --deliverable verdict --host laptop
```

With `--sessions operator` instead, no pane opens and that line is yours to
run: the operator is the session (93b).

**The merge, the close, the landing.** The stage passes on `done`; the item's
place merges into the bolt's line and is removed; and the bolt's close is the
one decision on the rail, with the number the register gave it (37, 39, 15):

```
approve
  1   bolt/flywheel-next/readme-crates-table     1 unit
      [ yes ]  [ no ]  [ later ]
```

Tap `yes` — on the laptop or on the phone, the same control either way. It
posts to the same tool a numbered chat reply calls, the answer is recorded at
once with who gave it and when, and on the next tick the line lands on `main`,
is pushed to the git host, and the Construction lane shows the landed record
(49, 153, 154, 193, 311).

**Read it all back.** Every write is a commit, and the record is readable with
nothing running (160, 167):

```sh
git -C ~/flywheel/git-host/agentplot-flywheel-next.git log --oneline main -3
git -C ~/flywheel/git-host/agentplot-state.git log --oneline
```

The first shows the chore's commit and the acceptance the landing wrote above
it; `git pull ~/flywheel/git-host/agentplot-flywheel-next.git main` brings them
into this checkout. The second is the capture, the unit, the work item, the
session, the answer and the status projection, each a commit with the reason
and the evidence the guard read in the message (79, 127). Nothing in either was
written by hand.

## The conformance suite

```sh
cargo run -q -- scenario run conformance/            # the whole phase-1 set
cargo run -q -- scenario run conformance/scenarios/S16.yaml --trace
```

`--profile` names the state store binding; `git-only` is this release's only one
and its default, and the trace goes to `target/flywheel-trace/git-only/` (92,
D15).

## What a session reports

An agent in a Herdr pane is the session, or the operator is (`--sessions
operator`, 93b); either reports the same way. A work order names the session
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
| `flywheel-workspace-host` | `Workspace` over git, on the clones a host keeps: a line is a branch, a place is a worktree, a landing pushes the shared line |
| `flywheel-sessions-operator` | `Sessions` with the operator as the session (93b) |
| `flywheel-sessions-herdr` | `Sessions` over Herdr, one `herdr agent start` in a pane at the place (196) |
| `flywheel-surface` | the page, the chat sink, the tool catalogue and its HTTP server |
| `flywheel-scenario` | the conformance runner, the scripted sessions, the trace renderer |
| `flywheel` | the binary: `init`, `host`, `scenario`, `capture`, `exit`, `offer`, `note`, `refuse` |

## Working here

Read `AGENTS.md` first: where the design lives, what belongs in this repository,
and how work is done.
