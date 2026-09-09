# The shipped instruction set

Where the instructions live (91). Beside `machines/`, because they are the
same kind of thing: data the release ships, an instance overrides, and the
engine reads by name and version without reading a word of the text (88,
119).

The layout mirrors the blueprints prefix. Cut `flywheel/` off a file's
`path` and what is left is where the file sits here.

| here | in the blueprints | what it is |
| --- | --- | --- |
| `instructions/` | `flywheel/instructions/` | the default instructions of 120, the four that shape what every session writes |
| `schemas/` | `flywheel/schemas/` | one schema per deliverable of the shipped set (190): what the artifact must contain and what makes it invalid |
| `schemas/units/<type>/<step>.md` | same | the `by-type` schema instruction of a unit type's stage, one per OpenSpec step |
| `skills/producers/<deliverable>/SKILL.md` | same | how a deliverable is written, against its schema |
| `skills/<agent>/SKILL.md` | same | the type skill, keyed by the agent name a machine or a stage names |
| `agents/<agent>.md` | same | the agent definition that skill belongs to |

`set.yaml` carries the release's set version, which default instructions a
type is handed when its file names none, and how a work order's inputs
resolve to files here.

Every file carries front matter — `name`, `kind`, `path`, `version`, `set`
— and `machines/check.py` fails a file whose front matter is missing, whose
`path` and location disagree, whose `set` is later than the release's, or
whose text moved while its `version` stood still (123). The hashes are
recorded in `machines/registry.yaml` under `instructions:`, written by
`check.py --register`, which is the registration of 224 for these files.

Changing one of these is a chore on the blueprints (91, 123). Hosts read
the blueprints' shared line, so the change reaches every host at its next
fetch; a session started before it carries the older version in its work
order's header and is unaffected.
