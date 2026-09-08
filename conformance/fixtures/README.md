# Fixtures

Every path a scenario names in `given.files`, in a `files` step, or inside a
`direct` command resolves against this directory. A path that names a file
here is materialized into the scenario's temporary checkout before the run;
a `files` value that is not a path here is taken as inline content for the
path it is keyed by, so a one-line file needs no file of its own. A path
resolving to neither is invalid and the run exits 2.

Nothing here is read by the machinery at run time. These are the world's
inputs: what an adapter finds, what a repository holds, what a session is
handed.

| path | used by | what it is |
|---|---|---|
| `meeting/2026-09-02-willdan-weekly.vtt` | S22 | a WebVTT transcript of one willdan weekly, cued by speaker. The meeting adapter reads it twice in S22 and must write one capture (111). It carries the material three of the suite's signals cite: the one-writer claim scoped to the wrong element, the status rows losing their numbers after a merge, and a retry bound hit twice on a flaky fixture. |
