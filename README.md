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
```

Crates: `flywheel-engine` (definitions, guards, tick, decisions, rec
format; no domain name in it), `flywheel-scenario` (the stand-in store and
world, scenario seeding), `flywheel-surface` (axum page and API),
`flywheel` (the binary).

What is real: the engine, the register, the tail, every effect the
machines ask for. What is faked: sessions (a script says what each would
report), git (places and lines are facts in the store), hosts (a heartbeat
per tick).
