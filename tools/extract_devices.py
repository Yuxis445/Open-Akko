import re, json, sys
src = open(sys.argv[1], encoding='utf-8').read()
starts = [m.start() for m in re.finditer(r'\{id:-?\d+,vid:\d+,pid:\d+', src)]
out = []
for i, s in enumerate(starts):
    chunk = src[s: starts[i+1] if i+1 < len(starts) else s+3000][:3000]
    g = lambda pat: (re.search(pat, chunk) or [None, None])[1]
    out.append(dict(id=int(g(r'^\{id:(-?\d+)')), vid=int(g(r'vid:(\d+)')), pid=int(g(r'pid:(\d+)')),
        name=g(r'[,{]name:"([^"]+)"'), displayName=g(r'displayName:"([^"]*)"'), type=g(r'type:"([a-z]+)"'),
        company=g(r'company:"([^"]*)"'), keyLayout=g(r'keyLayout:[A-Za-z_$]+\.([A-Za-z0-9_]+)'),
        layer=int(g(r'layer:(\d+)') or 1), magnetism='magnetism:!0' in chunk))
json.dump(out, open(sys.argv[2], 'w'), ensure_ascii=False, indent=0)
akko = [d for d in out if (d['company'] or '').lower() == 'akko']
print(len(out), 'devices;', len(akko), 'akko')
