#!/usr/bin/env python3
"""Check the statechart model for internal consistency and for its trace to
the requirements.

    uv run --with pyyaml --with jsonschema python3 check.py

- every machine file validates against schema.json
- every `ev` and `do` name exists in atoms.yaml; every effect's proof is an evidence name
- every transition target is a state of the same region; every `enter` names a state of some template
- every `machine:` reference names a machine file (or a `$param`); a bare name resolves to the
  highest version present, `name@N` to that version; a core machine names an extensible one only
  pinned or as a listed exception
- every file carries `tier: core | extensible` matching its directory; a type file is named
  `<machine>@<version>.yaml`, two files never declare the same name and version, and a registered
  file's content hash (machines/registry.yaml, written by `check.py --register`) never moves
- every decision has a kind, group, answers and satisfies; decision kinds are collected for the rail catalogue
- every diagram in ../diagrams/*.svg names states (data-state="machine.state"), decision kinds
  (data-decision) and effects (data-effect) that exist, so a picture cannot drift from the runtime (83)
- every profile marked complete binds every evidence and effect name (140)
- the shipped instruction set in ../instructions/: every file carries front matter (name, kind,
  path, version, set, satisfies), its `path` and its place in the directory agree (the directory
  mirrors the blueprints prefix), its `set` is one this release ships, and its text moves only
  when its version moves — the hashes are in registry.yaml under `instructions:` (88, 119, 123,
  224). Every `flywheel/{schemas,instructions,skills,agents}/…` path a profile names resolves to
  a file there; every default instruction set.yaml or a type's `instructions:` names resolves;
  and every agent a machine or a stage names has both a skill and a definition (89, 190)
- every conformance scenario validates against ../conformance/schema.json and names only real decision
  kinds and effects; every asserted transition's from and to are states of one region of the object's
  machine, submachines expanded, and `region:` on the entry says which when a name is ambiguous; every `then.state_store` key is bound in ../conformance/observations.yaml, for
  every profile the scenario runs on, and every bound key is asserted by some scenario; `hooks:` is
  declared only by a scenario under conformance/contract/
- the requirement trace (section 12 of the requirements): every machine, decision kind, effect and
  conformance scenario carries `satisfies: [numbers]`; a number that names no requirement fails; a
  requirement cited nowhere fails. The requirement numbers are read from ../../../requirements.md.
  A suffixed requirement (216a, 217b) is a clause of its base number: `satisfies` stays integer
  (schema.json, and the engine reads Vec<u32>), and the clause counts as cited when its base is.
Exit 1 on any finding.

`render.py` beside this file draws the same machine files as statecharts, one SVG per
machine under ../diagrams/machines/ with an index.md, so the operator reviews every
machine as a picture derived from its definition (83); run it after any change here:

    uv run --with pyyaml python3 machines/render.py
"""
import glob, os, re, sys, json
import yaml, jsonschema

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
REQUIREMENTS = os.path.normpath(os.path.join(ROOT, '..', '..', 'requirements.md'))
bad = []

# ---- the requirements: numbered items under ### headings
req_numbers = set()
req_section = {}
req_clauses = {}   # base number -> [216a, 217b, ...]
if os.path.exists(REQUIREMENTS):
    section = None
    for line in open(REQUIREMENTS):
        h = re.match(r'^### ([ABC]\.\d+) ', line)
        if h:
            section = h.group(1); continue
        if line.startswith('## '):
            section = None; continue
        n = re.match(r'^(\d{1,3})([a-z]?)\. ', line)
        if n and section:
            k = int(n.group(1)); req_numbers.add(k); req_section[k] = section
            if n.group(2):
                req_clauses.setdefault(k, []).append(n.group(1) + n.group(2))
else:
    bad.append(f"requirements not found at {REQUIREMENTS}")
cited = {}   # number -> [where]

def cite(nums, where):
    if not isinstance(nums, list) or not nums:
        bad.append(f"{where}: satisfies missing or empty"); return
    for n in nums:
        if not isinstance(n, int) or n not in req_numbers:
            bad.append(f"{where}: satisfies cites {n!r}, which names no requirement")
        else:
            cited.setdefault(n, []).append(where)

schema = json.load(open(os.path.join(HERE, 'schema.json')))
atoms = yaml.safe_load(open(os.path.join(HERE, 'atoms.yaml')))
evidence = set(atoms['evidence'])
effects = atoms['effects']
cite(atoms.get('satisfies'), 'atoms.yaml')

# ---- the machine files and the type registry (model.md 10.7)
# Core machines live in machines/ and machines/engine/ as <machine>.yaml. Extensible
# machines — unit types and elaboration types — live in machines/unit-types/ and
# machines/elaboration-types/ as <machine>@<version>.yaml: a type is addressed as
# name@version, a new version is a new file, and a registered file is never edited.
# machines/registry.yaml records the content hash of every extensible file at
# registration (`check.py --register` adds the unregistered ones); a hash that no
# longer matches fails. A `machine:` reference resolves a bare name to the highest
# version present and `name@N` to that version; `$param` references are the object's
# record and are resolved by the engine.
EXTENSIBLE_DIRS = {'unit-types', 'elaboration-types'}
BARE_REFERENCE_EXCEPTIONS = {'with-operator'}   # the operator's own session's type, named by a core machine (69)
REGISTRY = os.path.join(HERE, 'registry.yaml')
registry = (yaml.safe_load(open(REGISTRY)) if os.path.exists(REGISTRY) else None) or {}
registered = registry.get('types') or {}
register = '--register' in sys.argv
unregistered = {}

def sha256(path):
    import hashlib
    return hashlib.sha256(open(path, 'rb').read()).hexdigest()

machines = {}          # name -> (path, m): the highest version of each name
versions = {}          # name -> {version: (path, m)}
tiers = {}             # name -> tier
type_dirs = {}         # name -> the extensible directory it lives in
for path in sorted(glob.glob(os.path.join(HERE, '**', '*.yaml'), recursive=True)):
    if os.path.basename(path) in ('atoms.yaml', 'registry.yaml'):
        continue
    rel = os.path.relpath(path, HERE)
    subdir = os.path.dirname(rel)
    m = yaml.safe_load(open(path))
    try:
        jsonschema.validate(m, schema)
    except jsonschema.ValidationError as e:
        bad.append(f"{rel}: schema: {e.message} at {'/'.join(map(str, e.path))}")
        continue
    name, version, tier = m['machine'], m['version'], m['tier']
    stem = os.path.basename(rel)[:-len('.yaml')]
    if subdir in EXTENSIBLE_DIRS:
        if tier != 'extensible':
            bad.append(f"{rel}: a file under {subdir}/ must be tier extensible, not {tier}")
        if m['kind'] != 'template':
            bad.append(f"{rel}: an extensible machine must be kind template, not {m['kind']}")
        if stem != f"{name}@{version}":
            bad.append(f"{rel}: a type file is named <machine>@<version>.yaml; this one declares {name}@{version}")
        key = f"{name}@{version}"
        if version in versions.get(name, {}):
            bad.append(f"{rel}: {key} is already declared by {os.path.relpath(versions[name][version][0], HERE)}; a new version is a new file")
            continue
        h = sha256(path)
        if key in registered:
            if registered[key].get('sha256') != h:
                bad.append(f"{rel}: {key} was edited after registration (sha256 {h[:12]}… ≠ registered {str(registered[key].get('sha256'))[:12]}…); a change is a new file at a new version")
        else:
            unregistered[key] = {'file': rel, 'sha256': h}
            if not register:
                bad.append(f"{rel}: {key} is not registered; run `check.py --register` to record its content hash in registry.yaml")
    else:
        if tier != 'core':
            bad.append(f"{rel}: a file under machines/{subdir or ''} must be tier core, not {tier}")
        if stem != name:
            bad.append(f"{rel}: a core machine file is named <machine>.yaml; this one declares {name}")
        if 'retired' in m:
            bad.append(f"{rel}: retired is for extensible machines only")
    versions.setdefault(name, {})[version] = (path, m)
    tiers[name] = tier
    if subdir in EXTENSIBLE_DIRS:
        type_dirs[name] = subdir
    cite(m.get('satisfies'), f"machine {name}@{version}" if tier == 'extensible' else f"machine {name}")
for name, vs in versions.items():
    machines[name] = vs[max(vs)]
for key, entry in registered.items():
    n, _, v = key.rpartition('@')
    if not v.isdigit() or int(v) not in versions.get(n, {}):
        bad.append(f"registry.yaml: {key} is registered but no file declares it; a registered version is retired with `retired: true`, never deleted")
# ---- the shipped instruction set (model.md 10.6, instructions/README.md)
# The schemas, the default instructions and the skills live in ../instructions/,
# laid out as the blueprints prefix: cut `flywheel/` off a file's `path` and what
# is left is where the file sits. Every file carries front matter — name, kind,
# path, version, set, satisfies — and the registry records its hash, so text that
# moves while its version stands still is a finding (123). Nothing here is read
# by the engine: the engine resolves a name to a path and a version (119).
INSTRUCTIONS = os.path.join(ROOT, 'instructions')
INSTRUCTION_KINDS = {'instruction', 'schema', 'skill', 'agent'}
PERSONA_AGENT = 'tester'   # a persona is data in the order; the agent is the stage's own (profiles/context.yaml ruling 20)
instructions = {}      # flywheel/<path> -> {name, kind, version, file}: a name is unique per kind,
                       # a path is unique outright, so the path is the key and the registry's too
set_version = None
registered_instr = registry.get('instructions') or {}
unregistered_instr = {}
if not os.path.isdir(INSTRUCTIONS):
    bad.append("instructions/ not found; it is where the instructions live (91)")
else:
    iset_path = os.path.join(INSTRUCTIONS, 'set.yaml')
    iset = yaml.safe_load(open(iset_path)) if os.path.exists(iset_path) else None
    if not iset:
        bad.append("instructions/set.yaml not found; the release's set version and the defaults a type carries live there (224)")
    else:
        set_version = iset.get('set')
        if not isinstance(set_version, int) or set_version < 1:
            bad.append("instructions/set.yaml: `set` must be the release's set version, an integer (224)")
    for path in sorted(glob.glob(os.path.join(INSTRUCTIONS, '**', '*.md'), recursive=True)):
        rel = os.path.relpath(path, INSTRUCTIONS)
        if rel == 'README.md':
            continue
        text = open(path).read()
        m = re.match(r'^---\n(.*?)\n---\n', text, re.S)
        if not m:
            bad.append(f"instructions/{rel}: no front matter; every file carries name, kind, path, version, set and satisfies (224)")
            continue
        fm = yaml.safe_load(m.group(1)) or {}
        where = f"instruction {fm.get('name', rel)}"
        for k in ('name', 'kind', 'path', 'version', 'set'):
            if k not in fm:
                bad.append(f"instructions/{rel}: front matter has no {k}")
        cite(fm.get('satisfies'), where)
        if 'name' not in fm or 'path' not in fm or 'version' not in fm or 'set' not in fm:
            continue
        if fm['kind'] not in INSTRUCTION_KINDS:
            bad.append(f"instructions/{rel}: kind {fm['kind']!r} is not one of {sorted(INSTRUCTION_KINDS)}")
        if fm['path'] != 'flywheel/' + rel:
            bad.append(f"instructions/{rel}: path {fm['path']} and the file's place disagree; the directory mirrors the prefix, so path must be flywheel/{rel}")
        if not isinstance(fm['version'], int) or fm['version'] < 1:
            bad.append(f"instructions/{rel}: version must be an integer from 1 (123)")
        if set_version is not None and (not isinstance(fm['set'], int) or not 1 <= fm['set'] <= set_version):
            bad.append(f"instructions/{rel}: set {fm['set']!r} is not a set this release ships; the release's is {set_version} (224)")
        if not text[m.end():].strip():
            bad.append(f"instructions/{rel}: front matter and no text")
        if fm['path'] in instructions:
            bad.append(f"instructions/{rel}: {fm['path']} is already declared by {instructions[fm['path']]['file']}")
            continue
        instructions[fm['path']] = {'name': fm['name'], 'kind': fm['kind'], 'version': fm['version'], 'file': rel}
        h = sha256(path)
        prev = registered_instr.get(fm['path'])
        if prev and prev.get('version') == fm['version'] and prev.get('sha256') != h:
            bad.append(f"instructions/{rel}: the text moved and version is still {fm['version']}; changing an instruction moves its version, so a session started before it and one after can be told apart (123)")
        elif not prev or prev.get('version') != fm['version'] or prev.get('sha256') != h:
            unregistered_instr[fm['path']] = {'file': rel, 'version': fm['version'], 'sha256': h}
            if not register:
                bad.append(f"instructions/{rel}: {fm['name']}@{fm['version']} is not registered; run `check.py --register` to record it in registry.yaml (224)")
    for ipath in registered_instr:
        if ipath not in instructions:
            bad.append(f"registry.yaml: instruction {ipath} is registered but no file declares it")

    # every path a profile names resolves to a file here; a placeholder like
    # `flywheel/schemas/units/<type>/<step>.md` names no one file and is not matched
    NAMED_PATH = re.compile(r'flywheel/(?:schemas|instructions|skills|agents)/[A-Za-z0-9@/._-]+\.md')
    for prof in sorted(glob.glob(os.path.join(ROOT, 'profiles', '*.yaml'))):
        for ref in sorted(set(NAMED_PATH.findall(open(prof).read()))):
            if ref not in instructions:
                bad.append(f"{os.path.basename(prof)}: names {ref}, which the shipped instruction set has not (88, 119)")

    def resolve_instruction(ref, where):
        """a default instruction named by a type or by set.yaml: `name` or `name@version`"""
        n, at, v = ref.rpartition('@')
        n = n if at else ref
        e = instructions.get(f"flywheel/instructions/{n}.md")
        if e is None:
            bad.append(f"{where}: names {ref}, which the shipped instruction set has not")
        elif at and v.isdigit() and int(v) != e['version']:
            bad.append(f"{where}: names {ref}, but the set carries {n}@{e['version']}")

    # the defaults a type carries when its file names none (120), and the type's own list
    carries = (iset or {}).get('carries') or {}
    for key in ('elaboration-types', 'unit-types'):
        for ref in carries.get(key) or []:
            resolve_instruction(ref, f"set.yaml carries.{key}")
    for tname, refs in (carries.get('by-name') or {}).items():
        if tname not in versions:
            bad.append(f"set.yaml carries.by-name: {tname} names no machine")
        for ref in refs or []:
            resolve_instruction(ref, f"set.yaml carries.by-name.{tname}")
    for name, vs in versions.items():
        for v, (path, m) in vs.items():
            for ref in m.get('instructions') or []:
                resolve_instruction(ref, f"machine {name}@{v} instructions")
            if 'instructions' in m and tiers.get(name) != 'extensible':
                bad.append(f"machine {name}: instructions is for extensible machines only")

    # every agent a machine or a stage names has a skill and a definition (89, 190)
    def agents_of(m, mname):
        out = set()
        def params(st):
            p = (st or {}).get('params') or {}
            for key in ('agent', 'agents'):
                v = p.get(key)
                for a in ([v] if isinstance(v, str) else v if isinstance(v, list) else []):
                    if not isinstance(a, str) or a.startswith('$'):
                        continue
                    if a != 'by-type':
                        out.add(a)
                    elif tiers.get(mname) == 'extensible':
                        out.add(mname)          # a type's own by-type agent is the type (context.yaml ruling 3)
                    else:
                        out.update(n for n, d in type_dirs.items() if d == 'elaboration-types')
                if isinstance(v, dict) and v.get('rule'):
                    out.add(PERSONA_AGENT)
        def walk(region):
            for sn, st in (region.get('states') or {}).items():
                params(st)
                for rg in ((st or {}).get('regions') or {}).values():
                    walk(rg)
        for region in m['regions'].values():
            walk(region)
        return out
    for name, vs in versions.items():
        for v, (path, m) in vs.items():
            for a in sorted(agents_of(m, name)):
                for want, kind in ((f"flywheel/skills/{a}/SKILL.md", 'skill'), (f"flywheel/agents/{a}.md", 'agent')):
                    if want not in instructions:
                        bad.append(f"machine {name}@{v}: names agent {a}, whose {kind} {want} the instruction set has not (89)")

if register and (unregistered or unregistered_instr):
    registered.update(unregistered)
    registry['types'] = dict(sorted(registered.items()))
    if unregistered_instr:
        registered_instr.update(unregistered_instr)
        registry['instructions'] = dict(sorted(registered_instr.items()))
    registry.setdefault('doc', 'the content hash of every extensible machine file at registration; check.py fails a file whose hash moved (model.md 10.7). Add a version with `check.py --register`; never edit an entry')
    with open(REGISTRY, 'w') as f:
        f.write('# The type registry: name@version -> the file and its sha256 at registration, and\n')
        f.write('# the instruction set: name -> the file, its version and its sha256 (224).\n')
        f.write('# Written by `check.py --register`; a registered type file is immutable, and an\n')
        f.write("# instruction's text moves only with its version (model.md 10.6, 10.7).\n")
        yaml.safe_dump(registry, f, sort_keys=False, width=200)
    print(f"registered {len(unregistered)} types, {len(unregistered_instr)} instructions")

def resolve_machine_ref(ref):
    """a `machine:` reference -> the (name, version) it names, or None when it names nothing"""
    if ref.startswith('$'):
        return None
    n, at, v = ref.rpartition('@')
    if at and v.isdigit():
        return (n, int(v)) if int(v) in versions.get(n, {}) else None
    return (ref, max(versions[ref])) if ref in versions else None

for name, spec in effects.items():
    if spec.get('proof') and spec['proof'] not in evidence:
        bad.append(f"atoms: effect {name} proves by {spec['proof']} which is not an evidence name")
    cite(spec.get('satisfies'), f"effect {name}")

decisions = {}
def walk_guard(g, where):
    if not isinstance(g, dict):
        bad.append(f"{where}: guard not a mapping: {g!r}"); return
    for k, v in g.items():
        if k in ('all', 'any'):
            for x in v: walk_guard(x, where)
        elif k == 'not':
            walk_guard(v, where)
        elif k == 'ev':
            if v not in evidence: bad.append(f"{where}: unknown evidence {v}")
            for kk in ('eq_ev', 'ne_ev', 'gte_ev', 'lt_ev', 'gt_ev'):
                if kk in g and g[kk] not in evidence: bad.append(f"{where}: unknown evidence {g[kk]}")

def walk_effects(effs, where):
    for e in effs or []:
        if e['do'] not in effects: bad.append(f"{where}: unknown effect {e['do']}")

def walk_region(mname, rname, region, where):
    states = region['states']
    if region['initial'] not in states:
        bad.append(f"{where}: initial {region['initial']} not a state")
    for sname, st in states.items():
        sw = f"{where}.{sname}"
        st = st or {}
        if 'decision' in st:
            d = st['decision']; decisions.setdefault(d['kind'], []).append(f"{mname}.{sname}")
            cite(d.get('satisfies'), f"decision {d['kind']} at {mname}.{sname}")
        walk_effects(st.get('entry'), sw); walk_effects(st.get('exit'), sw)
        mref = st.get('machine')
        if mref and not mref.startswith('$'):
            target = resolve_machine_ref(mref)
            if target is None:
                bad.append(f"{sw}: submachine {mref} has no definition")
            elif '@' not in mref and tiers.get(mname) == 'core' and tiers.get(target[0]) == 'extensible' and target[0] not in BARE_REFERENCE_EXCEPTIONS:
                bad.append(f"{sw}: core machine {mname} names extensible {mref} by bare name; pin a version or list the exception")
            elif target and versions[target[0]][target[1]][1].get('retired') and '@' not in mref:
                bad.append(f"{sw}: {mref} resolves to a retired version {target[0]}@{target[1]}")
        for rn, rg in (st.get('regions') or {}).items():
            walk_region(mname, rn, rg, f"{sw}[{rn}]")
        for i, t in enumerate(st.get('transitions') or []):
            tw = f"{sw}.transitions[{i}]"
            walk_guard(t['when'], tw)
            walk_effects(t.get('effects'), tw)
            if t['to'] not in states:
                bad.append(f"{tw}: target {t['to']} not a state of region {rname}")

for mname, (path, m) in machines.items():
    for rname, region in m['regions'].items():
        walk_region(mname, rname, region, f"{mname}[{rname}]")

# enter targets and qualified finals: the named state must exist in some template's regions
template_states = {}
for mname, (path, m) in machines.items():
    def collect(region):
        for s, st in region['states'].items():
            template_states.setdefault(mname, set()).add(s)
            for rg in (st or {}).get('regions', {}).values(): collect(rg)
    for region in m['regions'].values(): collect(region)
all_states = set().union(*template_states.values())

# region paths and the states in each, per machine, with submachines expanded. A
# `$param` reference is the object's own type, resolved at run time; here it stands
# for every extensible machine, since any of them may be the one. Used by the
# conformance check that a transition's from and to are states of one region.
_regions_cache = {}
def region_states(mname, _stack=()):
    """[(region path, {states})] for a machine. A state that hosts a submachine contributes
    the submachine's own regions under `<path>.<state>.<subregion>`, and its states belong to
    those and not to the hosting region: leaving a stage is not leaving the state that runs it."""
    if mname in _regions_cache: return _regions_cache[mname]
    entry = machines.get(mname)
    if entry is None or mname in _stack: return []
    m = entry[1]
    out = []
    def subs_of(st):
        ref = (st or {}).get('machine')
        if ref and ref.startswith('$'):
            return [n for n in machines if tiers.get(n) == 'extensible']
        if ref:
            t = resolve_machine_ref(ref)
            return [t[0]] if t else []
        return []
    def walk(rs, prefix):
        for rn, r in (rs or {}).items():
            path = f"{prefix}{rn}"
            names = set()
            for sn, st in (r.get('states') or {}).items():
                names.add(sn)
                st = st or {}
                for sub in subs_of(st):
                    for subpath, substates in region_states(sub, _stack + (mname,)):
                        out.append((f"{path}.{sn}.{subpath}", substates))
                walk(st.get('regions'), f"{path}.{sn}.")
            out.append((path, names))
    walk(m['regions'], '')
    _regions_cache[mname] = out
    return out

def enters(obj, where):
    for k, v in (obj.get('enter') or {}).items():
        if v not in all_states: bad.append(f"{where}: enter {k}: {v} names no state")
def finals(g, where):
    if not isinstance(g, dict): return
    for k, v in g.items():
        if k in ('all', 'any'):
            for x in v: finals(x, where)
        elif k == 'not': finals(v, where)
        elif k == 'final':
            _, _, sn = v.rpartition('.')
            if sn not in all_states: bad.append(f"{where}: final {v} names no state")
for mname, (path, m) in machines.items():
    def walk(region, where):
        for s, st in region['states'].items():
            st = st or {}
            enters(st, f"{where}.{s}")
            for i, t in enumerate(st.get('transitions') or []):
                enters(t, f"{where}.{s}.transitions[{i}]"); finals(t['when'], f"{where}.{s}.transitions[{i}]")
            for rg in st.get('regions', {}).values(): walk(rg, f"{where}.{s}")
    for region in m['regions'].values(): walk(region, mname)

# diagrams must not drift from the definitions (83)
for svg in sorted(glob.glob(os.path.join(ROOT, 'diagrams', '*.svg'))):
    s = open(svg).read()
    for ref in re.findall(r'data-state="([^"]+)"', s):
        mn, _, sn = ref.partition('.')
        if mn not in template_states or sn not in template_states[mn]:
            bad.append(f"{os.path.basename(svg)}: data-state {ref} names no state")
    for ref in re.findall(r'data-decision="([^"]+)"', s):
        if ref not in decisions: bad.append(f"{os.path.basename(svg)}: data-decision {ref} names no decision kind")
    for ref in re.findall(r'data-effect="([^"]+)"', s):
        if ref not in effects: bad.append(f"{os.path.basename(svg)}: data-effect {ref} names no effect")
    for ref in re.findall(r'data-row="([^"]+)"', s):
        bad.append(f"{os.path.basename(svg)}: data-row {ref} is the old vocabulary; use data-decision")

# profile bindings must be complete (137)
for prof in sorted(glob.glob(os.path.join(ROOT, 'profiles', '*.yaml'))):
    p = yaml.safe_load(open(prof))
    if p.get('format') != 'flywheel-profile/1':
        continue
    bound_ev = set(p.get('evidence', {}))
    bound_fx = set(p.get('effects', {}))
    inh = p.get("inherits") or []
    for b in ([inh] if isinstance(inh, str) else inh):
        base = yaml.safe_load(open(os.path.join(ROOT, "profiles", b + ".yaml")))
        bound_ev |= set(base.get("evidence", {})); bound_fx |= set(base.get("effects", {}))
    for e in sorted(bound_ev - evidence):
        bad.append(f"{os.path.basename(prof)}: binds evidence {e}, which no atom names")
    for f in sorted(bound_fx - set(effects)):
        bad.append(f"{os.path.basename(prof)}: binds effect {f}, which no atom names")
    if p.get('complete', True):
        for e in sorted(evidence - bound_ev):
            bad.append(f"{os.path.basename(prof)}: evidence {e} not bound")
        for f in sorted(set(effects) - bound_fx):
            bad.append(f"{os.path.basename(prof)}: effect {f} not bound")

# conformance scenarios must validate, name only real decisions and effects, and cite requirements
sschema_path = os.path.join(ROOT, 'conformance', 'schema.json')
obs_path = os.path.join(ROOT, 'conformance', 'observations.yaml')
ALL_PROFILES = {'stand-in', 'git-only', 'tracker'}
observations = {}
if os.path.exists(obs_path):
    observations = (yaml.safe_load(open(obs_path)) or {}).get('observations') or {}
    for k, spec in observations.items():
        if not (spec or {}).get('doc'):
            bad.append(f"observations.yaml: {k} has no doc")
        p_ = set((spec or {}).get('profiles') or [])
        if not p_ or not p_ <= (ALL_PROFILES | {'all'}):
            bad.append(f"observations.yaml: {k} names no profile, or one that is not a profile")
else:
    bad.append("conformance/observations.yaml not found; every state_store key must be bound there")
observed = set()
nscen = 0
if os.path.exists(sschema_path):
    sschema = json.load(open(sschema_path))
    for path in sorted(glob.glob(os.path.join(ROOT, 'conformance', '**', '*.yaml'), recursive=True)):
        if '/lamp/' in path or os.path.basename(path) == 'observations.yaml':
            continue
        rel = os.path.relpath(path, ROOT)
        sc = yaml.safe_load(open(path))
        try:
            jsonschema.validate(sc, sschema)
        except jsonschema.ValidationError as e:
            bad.append(f"{rel}: scenario schema: {e.message} at {'/'.join(map(str, e.path))}"); continue
        nscen += 1
        cite(sc.get('satisfies'), f"scenario {sc['scenario']}")
        # every state_store key is bound in observations.yaml, for every profile the scenario runs on (94)
        runs = ALL_PROFILES if 'all' in (sc.get('profiles') or []) else set(sc.get('profiles') or [])
        for k in (sc.get('then', {}).get('state_store') or {}):
            observed.add(k)
            spec = observations.get(k)
            if spec is None:
                bad.append(f"{rel}: state_store key {k} is bound in no observations.yaml entry")
                continue
            answers = ALL_PROFILES if 'all' in (spec.get('profiles') or []) else set(spec.get('profiles') or [])
            missing = runs - answers
            if missing:
                bad.append(f"{rel}: state_store key {k} runs on {sorted(missing)}, which do not answer it")
        # hooks force a race the machinery prevents; only a contract scenario may declare one
        if sc.get('hooks') and os.path.dirname(rel) != os.path.join('conformance', 'contract'):
            bad.append(f"{rel}: hooks are allowed only under conformance/contract/")
        if sc.get('machines'):
            continue  # a toy machine set; not checked against the flywheel's decisions
        then = sc.get('then', {})
        for r in then.get('decisions', []) or []:
            for k in (r.get('present') or []) + (r.get('absent') or []):
                if k not in decisions: bad.append(f"{rel}: decision kind {k} does not exist")
        for e in then.get('effects', []) or []:
            if e['do'] not in effects: bad.append(f"{rel}: effect {e['do']} does not exist")
        # a transition names one region: from and to are states of the same region of the
        # object's machine, submachines expanded. `region:` on the entry narrows which.
        obj_machine = {o['id']: o['machine'] for o in (sc.get('given', {}).get('objects') or [])}
        for t in then.get('transitions', []) or []:
            frm, to, oid = t.get('from'), t.get('to'), t.get('object')
            if frm is None or to is None: continue
            mname = obj_machine.get(oid) or (oid.split('/')[0] if oid else None)
            rs = region_states(mname) if mname else []
            if not rs:
                bad.append(f"{rel}: transition on {oid} names machine {mname!r}, which has no definition"); continue
            named = t.get('region')
            if named is not None:
                rs = [(p, st) for p, st in rs if p == named or p.endswith('.' + named) or p.split('.')[-1] == named]
                if not rs:
                    bad.append(f"{rel}: transition on {oid} names region {named!r}, which {mname} has not"); continue
            frm_in = sorted(p for p, st in rs if frm in st)
            to_in = sorted(p for p, st in rs if to in st)
            if not frm_in or not to_in or not (set(frm_in) & set(to_in)):
                bad.append(f"{rel}: transition on {oid} from {frm} to {to} crosses regions: "
                           f"from is in {frm_in or 'no region'}, to is in {to_in or 'no region'} of {mname}")
        for step in sc.get('when', []):
            w = step.get('response')
            if w and w.get('decision') and w['decision'].rsplit('/', 1)[-1] not in decisions:
                bad.append(f"{rel}: response decision kind {w['decision'].rsplit('/', 1)[-1]} does not exist")

for k in sorted(set(observations) - observed):
    bad.append(f"observations.yaml: {k} is bound but no scenario asserts it")

# the trace: every requirement cited somewhere
uncited = sorted(req_numbers - set(cited))
for n in uncited:
    bad.append(f"requirement {n} ({req_section[n]}) is cited nowhere")

nclauses = sum(len(v) for v in req_clauses.values())
print(f"scenarios: {nscen} · requirements: {len(req_numbers)} (+{nclauses} clauses) · cited: {len(cited)} · uncited: {len(uncited)}")
print(f"observations: {len(observations)} bound · {len(observed)} asserted")
print(f"machines: {len(machines)} · evidence: {len(evidence)} · effects: {len(effects)} · decision kinds: {len(decisions)}")
kinds = {k: sum(1 for e in instructions.values() if e['kind'] == k) for k in sorted(INSTRUCTION_KINDS)}
print(f"instructions: {len(instructions)} files at set {set_version} · " + " · ".join(f"{v} {k}" for k, v in kinds.items()))
for k, v in sorted(decisions.items()): print(f"  decision {k}: {', '.join(v)}")
for b in bad: print("FAIL", b)
sys.exit(1 if bad else 0)
