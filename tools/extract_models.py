import re, json, sys
fin, base, out = sys.argv[1], sys.argv[2], sys.argv[3]
src = open(fin, encoding='utf-8').read()
bsrc = open(base, encoding='utf-8').read()

def parse_body(body):
    d = {}
    for k in ('defaultMatrix', 'defaultFnMatrix', 'defaultFnMACMatrix'):
        m = re.search(r'\b' + k + r' = \[([0-9, \n]*)\]', body)
        if m: d[k] = [int(x) for x in m.group(1).replace('\n', ' ').split(',') if x.strip()]
    for k in ('DAZZLE', 'NORMAL', 'MAXSPEED', 'FEA_CMD_GET_REV'):
        m = re.search(r'\b' + k + r' = (\d+);', body)
        if m: d[k] = int(m.group(1))
    m = re.search(r'COMMONCOLOR = \{([^}]*)\}', body)
    if m: d['COMMONCOLOR'] = {int(a): int(b) for a, b in re.findall(r'(\d+): (\d+)', m.group(1))}
    if 'setReportRate' in body: d['reportRate'] = True
    return d

def classes(text):
    res = {}
    pos = [(m.start(), m.group(1), m.group(2)) for m in re.finditer(r'^class (\w+) extends (\w+) \{', text, re.M)]
    for i, (s, n, p) in enumerate(pos):
        e = pos[i+1][0] if i+1 < len(pos) else len(text)
        res[n] = (p, parse_body(text[s:e]))
    return res

bc = classes(bsrc)            # Z (base), J extends Z  -> CommonKbYc500 = J
root = {}
root.update(bc['Z'][1]); root.update(bc['J'][1])
cls = classes(src)
def resolve(n, seen=()):
    if n == 't' or n not in cls: return dict(root)
    p, body = cls[n]
    r = resolve(p); r.update(body); return r
models = {}
for name, c in re.findall(r'case "([^"]+)":\s*return new (\w+)\(', src):
    models[name] = resolve(c)
models['_generic'] = dict(root)
json.dump(models, open(out, 'w'))
print(len(models), 'models;', sum('reportRate' in m for m in models.values()), 'with report rate')
