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
client at http://127.0.0.1:4242/agentplot · no sign-in: the operators list holds one entry, chuck, and every call is given by it (253a)
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
press return. Nothing in the text is parsed; the submission is recorded once
as a capture with one signal of kind ask, and the signal stands on the rail
as decision 1, in your own words, with three answers: build, intent, drop
(19, 19a, 111, 193, 194).

**From your own client.** The host printed the one address to add to the
client you already talk to, and that it asks no sign-in while you are its one
operator (319, 320, 253a). With Claude Code, for one:

```sh
claude mcp add --transport http agentplot http://localhost:4242/agentplot
```

Ask it what needs you. It calls `rail`, and a client that renders the
flywheel's views shows the rail inline: the page's own cards, numbers and
controls, drawn by the bundle the host serves (293a, 322). Tap `build` there
and your client sends it back as the same `answer` the page's control posts,
recorded once with who gave it and when (321, 323). A client that renders no
views reads the rail in words and makes the same call (311). By hand, those
are two calls at that address; the first prints the capture's number, which
the second names:

```sh
curl -s --json '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"rail"}}' http://localhost:4242/agentplot
curl -s --json '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"answer","arguments":{"decision":<number>,"answer":"build"}}}' http://localhost:4242/agentplot
```

The answer is a commit in the state repository —
`git -C ~/flywheel/git-host/agentplot-state.git log --oneline --grep 'response client-'`
shows it as `response client-1 by chuck` — and nothing you said to your client
is in there: the conversation stays your client's (324).

**Build.** Tapped on the page, pressed as `b` with the card in hand, or sent
from your client, that one answer makes a chore unit on a bolt named from the
capture's first words — no name to type — and approves it: the bolt's line is a branch of `flywheel-next` on
the git host, the unit's one work item has a place, a worktree of that line
under the host's root, and the Construction lane shows the bolt with its unit
and the session under it (19a, 34, 42, 44, 12).

**The session.** On the next tick the item enters its `fix` stage and the host
starts the session: a Herdr workspace named for the bolt, a tab named for the
unit, and Claude Code in the tab's pane at the place, told to read
`.flywheel/work-order.md` there. The order carries the capture's words as the
job, what to deliver, the rules, the chore-fixer's skill, and the exact line to
report with (67, 89, 196). Watch it work in the pane. When it is done it
reports with that line, which from the place is:

```sh
FLYWHEEL_SESSION=work-item/flywheel-next/readme-crates-table-lacks/wi-1/fix/1 \
FLYWHEEL_STATE=~/flywheel/hosts/laptop/agentplot/flywheel-state/main \
  cargo run -q -- exit done --deliverable commits --deliverable verdict --host laptop
```

With `--sessions operator` instead, no pane opens and that line is yours to
run: the operator is the session (93b).

**The merge, the close, the landing.** The stage passes on `done`; the item's
place merges into the bolt's line and is removed; and the bolt's close is the
one decision on the rail, with the number the register gave it (37, 39, 15):

```
approve
  2   bolt/flywheel-next/readme-crates-table-lacks     1 unit
      [ yes ]  [ hold ]
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

To have that decision reach your phone as a Discord notification as well, add
the chat: [The chat on your phone](#the-chat-on-your-phone).

## The conformance suite

```sh
cargo run -q -- scenario run conformance/            # the whole phase-1 set
cargo run -q -- scenario run conformance/scenarios/S16.yaml --trace
```

`--profile` names the state store binding; `git-only` is this release's only one
and its default, and the trace goes to `target/flywheel-trace/git-only/` (92,
D15).

The run plays every scenario and prints a line for each. Its exit is the phase
gate: the twenty-one scenarios phase 1 accepts must pass, and none of them may
be skipped. A scenario a later phase takes up is played too, and if it fails or
does not validate the report says what it expected and what it got, under
`deferred`, without failing the run (168).

## The chat on your phone

Optional, and added to the walkthrough above at any point: the decisions on the
rail also arrive in a Discord channel, so your phone's own notification brings
them, each with a link back to it on the page, and a tap or a short reply there
answers it (152–155, 309).

**A bot for the channel.** At <https://discord.com/developers/applications>,
make an application and give it a bot. Under *Bot*, turn on *Message Content
Intent*, which is how the bot reads a numbered reply, and copy the bot's token
somewhere private. Under *OAuth2 → URL Generator*, tick the `bot` scope and the
*View Channels*, *Send Messages* and *Read Message History* permissions, open
the URL it makes, and add the bot to your server. With *Developer Mode* on
(*User Settings → Advanced*), right-click the channel and copy its id.

**Name the chat.** The same `init` as the walkthrough's, with three more flags.
The manifest records the channel and the name of the variable the token is in,
never the token (204, 207):

```sh
cargo run -q -- init \
  --instance agentplot --host laptop \
  --root ~/flywheel/hosts/laptop --git-host ~/flywheel/git-host \
  --repository flywheel-next --workspace host --sessions herdr \
  --manifest ~/flywheel/flywheel.yaml \
  --chat discord --channel <the channel's id> --token-from FLYWHEEL_DISCORD_TOKEN
```

A link in the channel opens on your phone when the host's address carries the
page's port. If `hosts.laptop.router.base` in `~/flywheel/flywheel.yaml` reads
`http://<your-computer>.local`, add it:

```yaml
hosts:
  laptop:
    router:
      base: http://<your-computer>.local:4242
```

**Place the token and restart the host.** Stop the host, put the token where
you said it would be, and start it again with the same command:

```sh
export FLYWHEEL_DISCORD_TOKEN=<the bot's token>
cargo run -q -- host --name laptop --manifest ~/flywheel/flywheel.yaml \
  --serve 4242 --operator chuck
```

The host says it presents the chat. If the variable is empty, it says the chat
is under attention and why, and serves the page as before (217f):

```
presents chat on discord
```

**The same decision, in the channel.** The standing decision arrives as one
line — its number, its kind, the object, its answers and its link — with the
tail since the last delivery and a link to the page after it, and a button
beside the number for each answer:

```
#2 bolt-close · bolt/flywheel-next/readme-crates-table-lacks · yes | hold · http://<your-computer>.local:4242/agentplot/bolt/flywheel-next/readme-crates-table-lacks
http://<your-computer>.local:4242/agentplot
[ 2 yes ]  [ 2 hold ]
```

Tap `2 yes`, or reply `yes 2`. Either calls the same tool the page's control
does and is recorded once, with who gave it and when: the tap's
acknowledgement, `#2 → yes, recorded as …`, is shown to you alone, and a reply
gets the same line as a reply in the channel (137, 153, 154, 194). `git log` on
the state repository shows the one answer. A message forwarded into the channel
becomes a capture pointing back at it, and anything else typed there gets one
line back saying what the channel takes, and is not read (112, 194). The host
hears the channel while it runs, and when it starts again it reads what was sent
while it was away: a reply typed while the computer slept is recorded then, and
answered once, however many times the host restarts (217f, 137).

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
| `flywheel-surface` | the page, the chat sink, the tool catalogue, its HTTP server and the protocol server a member's own client reaches |
| `flywheel-scenario` | the conformance runner, the scripted sessions, the trace renderer |
| `flywheel` | the binary: `init`, `host`, `scenario`, `capture`, `exit`, `offer`, `note`, `refuse` |

## Working here

Read `AGENTS.md` first: where the design lives, what belongs in this repository,
and how work is done.
