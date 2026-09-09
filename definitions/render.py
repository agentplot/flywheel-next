#!/usr/bin/env python3
"""Render every machine under machines/ as a statechart, one SVG per machine,
into ../diagrams/machines/<machine>.svg (a type file as <machine>@<version>.svg), with an index.md beside them.

    uv run --with pyyaml python3 machines/render.py [--only a,b] [--png DIR]

Reads the same files check.py checks (every *.yaml under machines/ except
atoms.yaml, keyed by `machine:`) and draws what check.py validates: regions
as labelled bands, states as boxes — a decision state carries its kind as
an amber pill with its answers, a final state has a double border, a state
that runs a submachine names it — transitions as arrows labelled with the
guard in short form, the effects (⚙) and the `enter:` commands (↳), and a
state's own `enter:`, `entry:`, `exit:` and `tail:` as lines in its box.
Layout is one row per region, states ordered by longest path from the
initial state; forward transitions arc above the row, back transitions
below, each on a track of its own so no two labels overlap. The palette
and fonts are those of ../diagrams/flywheel-backlog.svg. Every rendering
is derived from the machine file, so the picture cannot drift from the
definition (83): re-run after any change to machines/.

`--png DIR` also rasterises each SVG with headless Chrome when one is
found, for a review outside the browser.
"""
import glob, html, os, re, shutil, subprocess, sys
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, 'diagrams', 'machines')

FONT = "-apple-system, 'Helvetica Neue', Arial, sans-serif"
INK, TEXT, DIM, FAINT, RULE = '#1a1a1a', '#333', '#666', '#999', '#e5e5e5'
BLUE, TEAL, PURPLE, RED, GREEN = '#4a7fb5', '#2b8a8a', '#7b5cb8', '#c05b5b', '#2e9e5b'
AMBER, AMBER_DARK, AMBER_FILL, GREY, PAPER = '#c9962e', '#8a6a1d', '#fffdf6', '#8a8a86', '#fafafa'
KIND_COLOR = {'object': BLUE, 'template': TEAL, 'engine': GREY}

TRACK = 15        # vertical distance between two edge tracks
GAP = 44          # horizontal gap between two state boxes
PAD = 10          # padding inside a composite box
LEFT = 30         # room for the initial-state marker
LABEL = 8.5       # font size of an edge label


def tw(s, size=LABEL, bold=False, mono=False):
    """Approximate rendered width of a string."""
    return len(s) * size * (0.62 if mono else (0.6 if bold else 0.55))


def esc(s):
    return html.escape(str(s), quote=True)


# ---- the loader: the same files check.py reads, keyed by `machine:`
def header_blurb(text):
    lines = []
    for line in text.splitlines():
        if line.startswith('#!'):
            continue
        if line.startswith('#'):
            lines.append(line.lstrip('#').strip())
        else:
            break
    blurb = ' '.join(l for l in lines if l)
    m = re.match(r'(.+?\.)(\s|$)', blurb)
    blurb = m.group(1) if m else blurb
    return blurb if len(blurb) <= 190 else blurb[:187] + '…'


def load_machines():
    machines = {}
    for path in sorted(glob.glob(os.path.join(HERE, '**', '*.yaml'), recursive=True)):
        if os.path.basename(path) in ('atoms.yaml', 'registry.yaml'):
            continue
        text = open(path).read()
        m = yaml.safe_load(text)
        m['_path'] = os.path.relpath(path, HERE)
        m['_blurb'] = header_blurb(text)
        # a type file is <machine>@<version>.yaml and draws as such, so two versions of one type
        # stand side by side; a core file draws by its machine name
        machines[os.path.basename(path)[:-len('.yaml')]] = m
    return machines


# ---- the short form of a guard
def guard_str(g, top=True):
    if not isinstance(g, dict):
        return str(g)
    if 'all' in g:
        s = ' ∧ '.join(guard_str(x, False) for x in g['all'])
        return s if top else f'({s})'
    if 'any' in g:
        s = ' ∨ '.join(guard_str(x, False) for x in g['any'])
        return s if top else f'({s})'
    if 'not' in g:
        inner = g['not']
        if isinstance(inner, dict) and 'ev' in inner and 'in' in inner:
            return f"{inner['ev']} ∉ {{{', '.join(map(str, inner['in']))}}}"
        if isinstance(inner, dict) and 'ev' in inner and inner.get('is') is True:
            return f"¬{inner['ev']}"
        s = guard_str(inner, False)
        return '¬' + (s if s.startswith('(') or ' ' not in s else f'({s})')
    if 'ev' in g:
        e = g['ev']
        if 'is' in g:
            v = g['is']
            return e if v is True else (f'¬{e}' if v is False else f'{e} = {v}')
        if 'in' in g:
            return f"{e} ∈ {{{', '.join(map(str, g['in']))}}}"
        if 'exists' in g:
            return f'{e} exists' if g['exists'] else f'no {e}'
        if 'older' in g:
            return f'{e} older than {g["older"]}'
        for k, sym in (('gte', '≥'), ('gt', '>'), ('lt', '<'), ('lte', '≤'),
                       ('eq_ev', '='), ('ne_ev', '≠'), ('gte_ev', '≥'), ('lt_ev', '<'), ('gt_ev', '>')):
            if k in g:
                return f'{e} {sym} {g[k]}'
        return e
    if 'response' in g:
        return f'reply {g["response"]}'
    if 'final' in g:
        return f'final {g["final"]}'
    if 'children' in g:
        c = g['children']; kind = c['kind']
        if 'all' in c:
            return f"∀{kind} ∈ {{{', '.join(c['all'])}}}"
        if 'any' in c:
            return f"∃{kind} ∈ {{{', '.join(c['any'])}}}"
        if 'none' in c:
            return f"∄{kind} ∈ {{{', '.join(c['none'])}}}"
        if 'count_gte' in c:
            return f'#{kind} ≥ {c["count_gte"]}'
        return f'children {kind}'
    if 'parent' in g:
        return f"parent ∈ {{{', '.join(g['parent'].get('in', []))}}}"
    if 'region' in g:
        r = g['region']
        return f"{r['name']} ∈ {{{', '.join(r['in'])}}}"
    if 'always' in g:
        return 'always'
    return '?'


def effects_str(effs):
    return ', '.join(e['do'] for e in effs or [])


def enter_str(enter):
    return ', '.join(f'{k}→{v}' for k, v in (enter or {}).items())


def guard_uses_response(g):
    if isinstance(g, dict):
        return 'response' in g or any(guard_uses_response(v) for v in g.values())
    if isinstance(g, list):
        return any(guard_uses_response(v) for v in g)
    return False


# ---- layout
class Box:
    def __init__(self, name):
        self.name = name
        self.x = self.y = self.w = self.h = 0
        self.lines = []      # (text, size, color, bold, mono)
        self.decision = None
        self.final = False
        self.machine = None
        self.subs = []       # nested RegionLayout, placed below the header
        self.header_h = 0
        self.tip = ''

    @property
    def cx(self):
        return self.x + self.w / 2


class Edge:
    def __init__(self, src, dst, label, color, tip):
        self.src, self.dst, self.label, self.color, self.tip = src, dst, label, color, tip
        self.above = True
        self.x1 = self.x2 = 0
        self.track = 0
        self.mx = 0
        self.lw = tw(label) + 8


class RegionLayout:
    def __init__(self, name, doc):
        self.name, self.doc = name, doc
        self.boxes = []      # in row order
        self.edges = []
        self.initial = None
        self.w = self.h = 0
        self.row_top = self.row_h = 0
        self.above = self.below = 0   # track counts


def order_states(region):
    names = list(region['states'])
    idx = {n: i for i, n in enumerate(names)}
    succ = {n: [] for n in names}
    for n in names:
        for t in ((region['states'][n] or {}).get('transitions') or []):
            v = t['to']
            if v != n and v in succ and v not in succ[n]:
                succ[n].append(v)
    color, back = {}, set()

    def dfs(u):
        color[u] = 1
        for v in succ[u]:
            if color.get(v) == 1:
                back.add((u, v))
            elif v not in color:
                dfs(v)
        color[u] = 2
    for s in [region['initial']] + names:
        if s not in color:
            dfs(s)
    fwd = {n: [v for v in succ[n] if (n, v) not in back] for n in names}
    preds = {n: [p for p in names if n in fwd[p]] for n in names}
    layer = {}

    def reach(u, seen):
        if u in seen:
            return seen
        seen.add(u)
        for v in fwd[u]:
            reach(v, seen)
        return seen
    for source in [region['initial']] + names:
        if source in layer:
            continue
        if fwd[source] and all(v in layer for v in fwd[source]):
            layer[source] = max(0, min(layer[v] for v in fwd[source]) - 1)
            continue
        cluster = reach(source, set())
        base = (max(layer.values()) + 1) if layer else 0

        def L(u):
            if u in layer:
                return layer[u]
            ps = [p for p in preds[u] if p in cluster and p != u]
            layer[u] = base if (u == source or not ps) else 1 + max(L(p) for p in ps)
            return layer[u]
        for u in sorted(cluster, key=lambda n: idx[n]):
            L(u)
    return sorted(names, key=lambda n: (layer[n], idx[n]))


def build_box(name, st, kind_color):
    st = st or {}
    b = Box(name)
    b.tip = st.get('doc') or ''
    b.lines.append((name, 11, INK, True, False))
    if 'decision' in st:
        d = st['decision']
        b.decision = d['kind']
        b.lines.append((d['kind'], 9, '#fff', False, True))
        b.lines.append(('answers: ' + ' · '.join(map(str, d['answers'])), 8.5, AMBER_DARK, False, False))
        if d.get('document'):
            b.lines.append((f"document {d['document']} on the review surface", 8, AMBER_DARK, False, False))
    if st.get('machine'):
        b.machine = st['machine']
        b.lines.append((f"⧉ {st['machine']}", 9.5, TEAL, True, False))
        if st.get('params'):
            ps = ' · '.join(f'{k}={param_str(v)}' for k, v in st['params'].items())
            for chunk in wrap(ps, 56):
                b.lines.append((chunk, 8, FAINT, False, False))
    if st.get('entry'):
        b.lines.append(('entry ⚙ ' + effects_str(st['entry']), 8.5, DIM, False, False))
    if st.get('exit'):
        b.lines.append(('exit ⚙ ' + effects_str(st['exit']), 8.5, DIM, False, False))
    if st.get('enter'):
        b.lines.append(('↳ enter ' + enter_str(st['enter']), 8.5, DIM, False, False))
    if st.get('tail'):
        b.lines.append((f"✓ tail: {st['tail']}", 8.5, GREEN, False, False))
    b.final = bool(st.get('final'))
    b.header_h = 8 + sum(14 if i == 0 else 12 for i in range(len(b.lines))) + 6
    inner_w = max(tw(t, s, bold, mono) for t, s, _, bold, mono in b.lines) + 22
    if st.get('regions'):
        b.subs = [build_region(rn, rg, kind_color) for rn, rg in st['regions'].items()]
        b.w = max(inner_w, max(s.w for s in b.subs) + 2 * PAD)
        b.h = b.header_h + sum(s.h + 16 for s in b.subs) + PAD
    else:
        b.w = max(96, inner_w)
        b.h = b.header_h
    return b


def param_str(v):
    if isinstance(v, list):
        return '[' + ', '.join(x['name'] if isinstance(x, dict) and 'name' in x else str(x) for x in v) + ']'
    if isinstance(v, dict):
        return '{' + ', '.join(f'{k}={param_str(x)}' for k, x in v.items()) + '}'
    return str(v)


def wrap(s, n):
    words, out, cur = s.split(' '), [], ''
    for w in words:
        if cur and len(cur) + 1 + len(w) > n:
            out.append(cur); cur = w
        else:
            cur = (cur + ' ' + w).strip()
    if cur:
        out.append(cur)
    return out


def build_region(name, region, kind_color):
    R = RegionLayout(name, region.get('doc') or '')
    R.initial = region['initial']
    order = order_states(region)
    boxes = {n: build_box(n, region['states'][n], kind_color) for n in order}
    x = LEFT
    for n in order:
        b = boxes[n]; b.x = x; x += b.w + GAP
        R.boxes.append(b)
    R.row_h = max(b.h for b in R.boxes)
    pos = {b.name: i for i, b in enumerate(R.boxes)}
    # edges
    for n in order:
        st = region['states'][n] or {}
        for t in st.get('transitions') or []:
            label = guard_str(t['when'])
            if t.get('effects'):
                label += ' ⚙ ' + effects_str(t['effects'])
            if t.get('enter'):
                label += ' ↳ ' + enter_str(t['enter'])
            if t.get('bump'):
                label += f" +{t['bump']}"
            color = AMBER if guard_uses_response(t['when']) else (kind_color if pos[t['to']] >= pos[n] else GREY)
            e = Edge(n, t['to'], label, color, t.get('note') or '')
            e.above = pos[t['to']] >= pos[n]
            R.edges.append(e)
    # ports: spread the attachment points along each box's top (above) or bottom (below)
    for side in (True, False):
        for b in R.boxes:
            ports = []
            for e in R.edges:
                if e.above != side:
                    continue
                if e.src == b.name and e.dst == b.name:
                    ports.append((b.cx - 1, e, 'src')); ports.append((b.cx + 1, e, 'dst'))
                elif e.src == b.name:
                    ports.append((boxes[e.dst].cx, e, 'src'))
                elif e.dst == b.name:
                    ports.append((boxes[e.src].cx, e, 'dst'))
            ports.sort(key=lambda p: p[0])
            k = len(ports)
            for i, (_, e, end) in enumerate(ports):
                px = b.cx if k == 1 else b.x + 10 + (b.w - 20) * i / (k - 1)
                if end == 'src':
                    e.x1 = px
                else:
                    e.x2 = px
    # tracks: an edge takes the lowest track whose intervals it does not overlap
    for side in (True, False):
        tracks = []
        es = [e for e in R.edges if e.above == side]
        es.sort(key=lambda e: (abs(e.x2 - e.x1), e.lw))
        for e in es:
            e.mx = max((e.x1 + e.x2) / 2, e.lw / 2 + 2)
            lo = min(e.x1, e.x2, e.mx - e.lw / 2) - 4
            hi = max(e.x1, e.x2, e.mx + e.lw / 2) + 4
            for i, tr in enumerate(tracks):
                if all(hi < a or lo > b_ for a, b_ in tr):
                    e.track = i; tr.append((lo, hi)); break
            else:
                e.track = len(tracks); tracks.append([(lo, hi)])
        if side:
            R.above = len(tracks)
        else:
            R.below = len(tracks)
    R.row_top = 18 + (R.above * TRACK + 8 if R.above else 0)
    R.w = max([b.x + b.w for b in R.boxes] + [e.mx + e.lw / 2 for e in R.edges]) + PAD
    R.h = R.row_top + R.row_h + (R.below * TRACK + 10 if R.below else 6)
    return R


# ---- drawing
def draw_region(R, ox, oy, kind_color, out, nested=False):
    label_color = FAINT if nested else GREY
    out.append(f'<text x="{ox}" y="{oy + 11}" font-size="{9.5 if nested else 11.5}" font-weight="700" fill="{label_color}">'
               f'{esc(R.name)}{tip(R.doc)}</text>')
    if not nested:
        out.append(f'<line x1="{ox + tw(R.name, 11.5, True) + 12:.0f}" y1="{oy + 7}" x2="{ox + R.w}" y2="{oy + 7}" stroke="{kind_color}" stroke-width="2" opacity="0.3"/>')
    row_y = oy + R.row_top
    # initial marker
    ib = next(b for b in R.boxes if b.name == R.initial)
    out.append(f'<circle cx="{ib.x + ox - 18:.1f}" cy="{row_y + ib.h / 2:.1f}" r="4" fill="{INK}"/>')
    out.append(f'<line x1="{ib.x + ox - 14:.1f}" y1="{row_y + ib.h / 2:.1f}" x2="{ib.x + ox - 1:.1f}" y2="{row_y + ib.h / 2:.1f}" stroke="{INK}" stroke-width="1.4" marker-end="url(#mI)"/>')
    # edges first, so boxes and labels sit on top
    boxes = {b.name: b for b in R.boxes}
    labels = []
    for e in R.edges:
        b1, b2 = boxes[e.src], boxes[e.dst]
        if e.above:
            y1, y2 = row_y + b1.y, row_y + b2.y
            ty = row_y - 8 - e.track * TRACK
        else:
            y1, y2 = row_y + b1.y + b1.h, row_y + b2.y + b2.h
            ty = row_y + R.row_h + 10 + e.track * TRACK
        x1, x2 = e.x1 + ox, e.x2 + ox
        marker = {AMBER: 'mA', GREY: 'mG', BLUE: 'mB', TEAL: 'mT'}.get(e.color, 'mB')
        out.append(f'<path d="M{x1:.1f},{y1:.1f} V{ty:.1f} H{x2:.1f} V{y2:.1f}" fill="none" stroke="{e.color}" '
                   f'stroke-width="1.3" marker-end="url(#{marker})">{tip(e.tip)}</path>')
        labels.append((e, e.mx + ox, ty))
    for b in R.boxes:
        draw_box(b, ox, row_y, kind_color, out)
    for e, mx, ty in labels:
        ly = ty - 3 if e.above else ty + 9
        out.append(f'<rect x="{mx - e.lw / 2:.1f}" y="{ly - 8.5:.1f}" width="{e.lw:.1f}" height="11" fill="#fff" opacity="0.92"/>')
        out.append(f'<text x="{mx:.1f}" y="{ly:.1f}" font-size="{LABEL}" fill="{TEXT}" text-anchor="middle">{esc(e.label)}{tip(e.tip)}</text>')


def tip(text):
    return f'<title>{esc(text)}</title>' if text else ''


def draw_box(b, ox, oy, kind_color, out):
    x, y = b.x + ox, b.y + oy
    stroke = AMBER if b.decision else (TEAL if b.machine else kind_color)
    fill = AMBER_FILL if b.decision else ('#fff' if not b.subs else PAPER)
    out.append(f'<rect x="{x:.1f}" y="{y:.1f}" width="{b.w:.1f}" height="{b.h:.1f}" rx="7" fill="{fill}" stroke="{stroke}" stroke-width="{2 if b.final else 1.6}">{tip(b.tip)}</rect>')
    if b.final:
        out.append(f'<rect x="{x + 3:.1f}" y="{y + 3:.1f}" width="{b.w - 6:.1f}" height="{b.h - 6:.1f}" rx="5" fill="none" stroke="{stroke}" stroke-width="1.2"/>')
    ty = y + 8
    for i, (text, size, color, bold, mono) in enumerate(b.lines):
        ty += 14 if i == 0 else 12
        if mono and b.decision and text == b.decision:
            pw = tw(text, 9, mono=True) + 14
            out.append(f'<rect x="{b.cx + ox - pw / 2:.1f}" y="{ty - 10:.1f}" width="{pw:.1f}" height="14" rx="7" fill="{AMBER}"/>')
            out.append(f'<text x="{b.cx + ox:.1f}" y="{ty:.1f}" font-size="9" font-family="monospace" fill="#fff" text-anchor="middle">{esc(text)}</text>')
        else:
            out.append(f'<text x="{b.cx + ox:.1f}" y="{ty:.1f}" font-size="{size}" fill="{color}" text-anchor="middle"'
                       f'{" font-weight=\"700\"" if bold else ""}>{esc(text)}</text>')
    sy = y + b.header_h
    for sub in b.subs:
        out.append(f'<line x1="{x + PAD:.1f}" y1="{sy + 4:.1f}" x2="{x + b.w - PAD:.1f}" y2="{sy + 4:.1f}" stroke="{RULE}"/>')
        draw_region(sub, x + PAD, sy + 8, kind_color, out, nested=True)
        sy += sub.h + 16


def render_machine(m):
    kind_color = KIND_COLOR.get(m['kind'], BLUE)
    regions = [build_region(rn, rg, kind_color) for rn, rg in m['regions'].items()]
    items = [(kind_color, 'state'), (AMBER, 'decision state · amber pill is the kind'), (TEAL, '⧉ runs a submachine'),
             (INK, 'double border: final'), (kind_color, 'arrow above: forward'), (GREY, 'arrow below: back'),
             (AMBER, 'amber arrow: the operator\'s response'), (DIM, '⚙ effects · ↳ enter commands · +bump · ● initial')]
    W = max(r.w for r in regions) + 80
    W = max(W, 56 + sum(tw(t, 9) + 34 for _, t in items) + 40)
    y = 96
    body = []
    for R in regions:
        draw_region(R, 40, y, kind_color, body)
        y += R.h + 26
    legend_y = y + 6
    H = legend_y + 44
    head = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W:.0f}" height="{H:.0f}" viewBox="0 0 {W:.0f} {H:.0f}" font-family="{FONT}">',
            '<defs>']
    for mid, col in (('mB', BLUE), ('mT', TEAL), ('mG', GREY), ('mA', AMBER), ('mI', INK)):
        head.append(f'<marker id="{mid}" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{col}"/></marker>')
    head.append('</defs>')
    head.append(f'<rect width="{W:.0f}" height="{H:.0f}" fill="#fff"/>')
    title = f"{m['machine']}@{m['version']} — {m['kind']}, {m['tier']}" if m['tier'] == 'extensible' else f"{m['machine']} — {m['kind']} v{m['version']}, {m['tier']}"
    if m.get('object'):
        title += f" · object {m['object']}"
    if m.get('singleton'):
        title += f" · one per {m['singleton']}"
    if m.get('parent'):
        p = m['parent']; title += f" · parent {', '.join(p) if isinstance(p, list) else p}"
    if m.get('owns'):
        title += f" · owns {', '.join(m['owns'])}"
    head.append(f'<text x="40" y="36" font-size="19" font-weight="700" fill="{INK}">{esc(title)}</text>')
    head.append(f'<text x="40" y="57" font-size="12" fill="{DIM}">{esc(m["_blurb"])}</text>')
    meta = f"satisfies {', '.join(map(str, m['satisfies']))} · {m['_path']}"
    if m.get('params'):
        meta += ' · params ' + ', '.join(m['params'])
    if m.get('record'):
        meta += ' · record ' + ', '.join(m['record'])
    head.append(f'<text x="40" y="75" font-size="9.5" font-family="monospace" fill="{FAINT}">{esc(meta)}</text>')
    # legend
    lg = [f'<rect x="40" y="{legend_y}" width="{W - 80:.0f}" height="30" rx="8" fill="{PAPER}" stroke="{RULE}"/>']
    lx = 56
    for col, text in items:
        lg.append(f'<rect x="{lx}" y="{legend_y + 10}" width="10" height="10" rx="2" fill="#fff" stroke="{col}" stroke-width="1.6"/>')
        lg.append(f'<text x="{lx + 15}" y="{legend_y + 19}" font-size="9" fill="{DIM}">{esc(text)}</text>')
        lx += tw(text, 9) + 34
    return '\n'.join(head + body + lg + ['</svg>']), (W, H)


def find_chrome():
    for c in ('google-chrome', 'chromium', 'chromium-browser', 'chrome'):
        p = shutil.which(c)
        if p:
            return p
    mac = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
    return mac if os.path.exists(mac) else None


def main(argv):
    only, png_dir = None, None
    it = iter(argv)
    for a in it:
        if a == '--only':
            only = set(next(it).split(','))
        elif a == '--png':
            png_dir = next(it)
    machines = load_machines()
    os.makedirs(OUT, exist_ok=True)
    sizes = {}
    for name, m in sorted(machines.items(), key=lambda kv: (os.path.dirname(kv[1]['_path']), kv[0])):
        if only and name not in only:
            continue
        svg, size = render_machine(m)
        with open(os.path.join(OUT, f'{name}.svg'), 'w') as f:
            f.write(svg)
        sizes[name] = size
    groups = {}
    for name, m in machines.items():
        groups.setdefault(os.path.dirname(m['_path']) or '.', []).append(m)
    lines = ['# Machines — rendered', '',
             'One statechart per machine file, drawn by `machines/render.py` from the definition',
             'itself, so the picture cannot drift from the runtime (83). Regenerate with',
             '`uv run --with pyyaml python3 machines/render.py` from `models/statechart`.', '',
             f'{len(machines)} machines: '
             + ', '.join(f"{sum(1 for m in machines.values() if m['kind'] == k)} {k}" for k in ('object', 'template', 'engine')) + '.', '']
    titles = {'.': 'Core objects and structural templates (`machines/`)', 'engine': 'Engine machines (`machines/engine/`)',
              'unit-types': 'Unit types (`machines/unit-types/<machine>@<version>.yaml`)', 'elaboration-types': 'Elaboration types (`machines/elaboration-types/<machine>@<version>.yaml`)'}
    for g in ('.', 'engine', 'unit-types', 'elaboration-types'):
        if g not in groups:
            continue
        lines += [f'## {titles[g]}', '', '| machine | kind | tier | version | object | satisfies | diagram |', '|---|---|---|---|---|---|---|']
        for m in sorted(groups[g], key=lambda m: (m['machine'], m['version'])):
            obj = m.get('object') or ('template' if m['kind'] == 'template' else '—')
            stem = os.path.basename(m['_path'])[:-len('.yaml')]
            shown = f"{m['machine']}@{m['version']}" if m['tier'] == 'extensible' else m['machine']
            retired = ' (retired)' if m.get('retired') else ''
            lines.append(f"| `{shown}`{retired} | {m['kind']} | {m['tier']} | {m['version']} | {obj} | {', '.join(map(str, m['satisfies']))} | [{stem}.svg]({stem}.svg) |")
        lines.append('')
    with open(os.path.join(OUT, 'index.md'), 'w') as f:
        f.write('\n'.join(lines))
    print(f"rendered {len(sizes)} machines to {os.path.relpath(OUT, ROOT)}/ · index.md lists {len(machines)}")
    if png_dir:
        chrome = find_chrome()
        if not chrome:
            print('no Chrome found; PNGs skipped'); return 0
        os.makedirs(png_dir, exist_ok=True)
        for name, (w, h) in sizes.items():
            subprocess.run([chrome, '--headless=new', '--disable-gpu', '--no-sandbox', '--hide-scrollbars',
                            f'--window-size={int(w)},{int(h)}', f'--screenshot={os.path.join(png_dir, name + ".png")}',
                            'file://' + os.path.join(OUT, f'{name}.svg')], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print(f'PNGs in {png_dir}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
