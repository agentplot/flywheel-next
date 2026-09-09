---
name: units/persona-test/ff
kind: schema
path: flywheel/schemas/units/persona-test/ff.md
version: 1
set: 1
satisfies: [37, 190]
---

# persona-test, the spec step

As the default type's spec step: `opsx ff` writes the change directory and the
spec from the work item.

One difference. This type's work will be exercised by personas, so every
scenario is written in terms of what a person doing the persona's job would
do and see, not in terms of the interface's internals.

Invalid: a scenario only the implementation could evaluate, and a spec that
names the personas instead of the behaviour they will find.
