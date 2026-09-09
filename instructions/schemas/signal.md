---
name: signal
kind: schema
path: flywheel/schemas/signal.md
version: 1
set: 1
satisfies: [111, 113, 114]
---

# A signal

One thing read from a capture, worth someone's judgment. Immutable once
written.

It carries the kind, who asserted it, the subject tags from the instance's
vocabulary, the assertion in the asserter's terms, the verbatim excerpt with
its position in the source, and the standing claims it argues with, if any.

The excerpt is quoted exactly, and the assertion never says more than the
excerpt supports. A signal is evidence, not an argument.

Invalid: a paraphrase presented as an excerpt, a signal with no position in
its source, a subject tag outside the vocabulary, a signal that copies the raw
material rather than pointing at it, and a signal that proposes what to do
about itself.
