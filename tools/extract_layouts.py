"""Extract per-layout key geometry from Akko Cloud's SVG keyboard components."""
import html, json, os, re, sys
JS = sys.argv[1]; PRETTY = sys.argv[2]; OUT = sys.argv[3]; WANT = set(sys.argv[4].split(','))
idx = open(os.path.join(PRETTY, 'index.js'), encoding='utf-8').read()

# 1) layout enum -> component var, from the RT keyMappings switch (case m.X: u = d.jsx(VAR, {}))
comp = {}
for m in re.finditer(r'case m\.(\w+):\s*u = d\.jsx\((\w+), \{\}\)', idx):
    comp.setdefault(m.group(1), m.group(2))
# 2) component var -> chunk file
chunk = {v: f for v, f in re.findall(r'(\w+) = x\.lazy\(\(\) => y\(\(\) => import\("\./([0-9a-f]+\.js)"\)', idx)}

def svg_rects(svg_js):
    s = open(os.path.join(JS, svg_js), encoding='utf-8').read()
    svg = s[s.find('<svg'):]
    first = svg.find('id="hover"')                 # keep only the "default" state
    svg = svg[:first] if first > 0 else svg
    keys, order = {}, []
    for m in re.finditer(r'<g id="#([^"]+)"[^>]*>(.*?)(?=<g id="#|\Z)', svg, re.S):
        code, body = html.unescape(m.group(1)), m.group(2)
        r = re.search(r'<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)"', body)
        c = re.search(r'<circle[^>]*? cx="([\d.]+)" cy="([\d.]+)" r="([\d.]+)"', body)
        p = re.search(r'<path d="M([\d.]+) ([\d.]+)', body)
        if code in keys: continue
        if r:
            x, y, w, h = map(float, r.groups()); keys[code] = dict(x=x, y=y, w=w, h=h)
        elif c:
            cx, cy, rr = map(float, c.groups()); keys[code] = dict(x=cx-rr, y=cy-rr, w=2*rr, h=2*rr, round=True)
        else:
            continue
        order.append(code)
    wh = re.search(r'<svg width="([\d.]+)" height="([\d.]+)"', svg)
    return dict(width=float(wh.group(1)), height=float(wh.group(2)),
                keys=[dict(code=k, **keys[k]) for k in order])

out = {}
for lay in sorted(WANT):
    v = comp.get(lay); f = chunk.get(v)
    if not f: print('no bundled component for', lay); continue
    src = open(os.path.join(JS, f), encoding='utf-8').read()
    sv = re.search(r'import\("\./([0-9a-f]+\.js)"\)', src)
    if not sv: print('no svg for', lay); continue
    out[lay] = svg_rects(sv.group(1))
    print(lay, '->', f, sv.group(1), len(out[lay]['keys']), 'keys')
json.dump(out, open(OUT, 'w'))
