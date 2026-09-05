# flywheel next — prototype

A first build of the flywheel redesign: the engine that reconciles declared
state machines, a stand-in control plane where sessions are played from a
script, and the plan page. The definitions under `definitions/` are copied
from the statechart model in the blueprints repo
(`design/flywheel-next/models/statechart/machines` and `profiles`); the
requirements are `design/flywheel-next/requirements.md` there.

```
cargo run -- seed scenarios/plan-mockup.yaml   # seed the store from a scenario
cargo run -- tick                              # tick until nothing fires
cargo run -- respond 416 yes                   # answer a numbered decision, then settle
cargo run -- dictate unit/atlas/plan-tail drop # a dictation on an object
cargo run -- plan                              # print the plan
cargo run -- log 60                            # the machinery log
cargo run -- serve --port 4242                 # the plan page on http://localhost:4242
cargo run -- dictate service/atlas/plan-rows/web start   # start a declared service in the bolt's place
```

Services: `given.services` in a scenario is what each repository's
`.flywheel/services.yaml` would declare; every open bolt with a ready place
gets one `service` object per declaration (stopped), and the page shows them
beside the bolt with their endpoint and a start/stop button, each a dictation
on the service object. In the stand-in a started process serves one tick
later at `http://<repo>.<bolt>.localhost:<port>`; `fails: true` on a
declaration scripts a process that exits instead, so the service reaches
`failed` with an attention decision.

The capture box at the top of the page (requirement 19): plain text posts
`/api/capture` and becomes a `capture` with one `signal` of kind ask;
`intent: …` proposes an intent, `chore <repo>: …` a chore unit on that
repository's shared line, `bolt <name>: …` a unit on that bolt. Each
submission is recorded once as a response naming the object it made.

Crates: `flywheel-engine` (definitions, guards, tick, decisions, rec
format; no domain name in it), `flywheel-scenario` (the stand-in store and
world, scenario seeding), `flywheel-surface` (axum page and API),
`flywheel` (the binary).

What is real: the engine, the register, the tail, every effect the
machines ask for. What is faked: sessions (a script says what each would
report), git (places and lines are facts in the store), hosts (a heartbeat
per tick).
