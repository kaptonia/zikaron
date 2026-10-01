#!/usr/bin/env python3
"""Run a candidate zk1 (ZK1_CANDIDATE) and the frozen Python criterion over the corpus and report every divergence."""
import json, os, subprocess, sys, collections, hashlib
ROOT = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.join(ROOT, 'corpus')
RUST = [os.environ.get('ZK1_CANDIDATE') or sys.exit('set ZK1_CANDIDATE to the candidate zk1 binary (HARNESS.md contract)')]
PY = ['python3', os.path.join(ROOT, 'impl-py', 'zk1.py')]

def run(cmd, args, timeout=60):
    try:
        p = subprocess.run(cmd + args, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return ('TIMEOUT', b'')
    if p.returncode not in (0,):
        return ('EXIT%d' % p.returncode, p.stdout[:400])
    return ('OK', p.stdout)

def files(sub, exts=None):
    d = os.path.join(CORPUS, sub)
    if not os.path.isdir(d):
        return []
    out = []
    for name in sorted(os.listdir(d)):
        p = os.path.join(d, name)
        if os.path.isfile(p) and not name.endswith('.note') and not name.endswith('.md'):
            if exts is None or any(name.endswith(e) for e in exts):
                out.append(p)
    return out

manifest = {}
mp = os.path.join(CORPUS, 'manifest.json')
if os.path.exists(mp):
    try:
        m = json.load(open(mp))
        entries = m if isinstance(m, list) else m.get('cases', m.get('files', []))
        for e in entries:
            if isinstance(e, dict) and 'path' in e:
                manifest[os.path.basename(e['path'])] = e
            elif isinstance(e, dict) and 'file' in e:
                manifest[os.path.basename(e['file'])] = e
    except Exception as ex:
        print('manifest unreadable:', ex)

summary = collections.Counter()
divergences = []
predmiss = []

def compare(cmd_name, argsf, sub, exts=None):
    for p in files(sub, exts):
        a = run(RUST, [cmd_name] + argsf(p))
        b = run(PY, [cmd_name] + argsf(p))
        summary[cmd_name] += 1
        if a != b:
            divergences.append((cmd_name, os.path.basename(p), a, b))
            summary[cmd_name + '_div'] += 1
        else:
            e = manifest.get(os.path.basename(p))
            if e and a[0]=='OK':
                try:
                    got=json.loads(a[1])
                    if cmd_name=='check':
                        pred=e.get('predicted'); got_t = 'ok' if got.get('ok') else got.get('token')
                        if pred and pred!=got_t:
                            summary['check_vs_prediction_diff'] += 1; predmiss.append((os.path.basename(p), pred, got_t, e.get('note')))
                    elif cmd_name=='audit':
                        pred=e.get('predicted_label'); got_l = got.get('label', got.get('reason'))
                        if pred and pred!=got_l:
                            summary['audit_vs_prediction_diff'] += 1; predmiss.append((os.path.basename(p), pred, got_l, e.get('note')))
                except Exception: summary[cmd_name+'_unparsable'] += 1

compare('check', lambda p: [p], 'canon')
compare('canon', lambda p: [p], 'canon')
# sign: corpus format sNNN.bin + sNNN.meta.json {privkey, domain, input}
sd = os.path.join(CORPUS, 'sign')
if os.path.isdir(sd):
    for name in sorted(os.listdir(sd)):
        if not name.endswith('.meta.json'):
            continue
        meta = json.load(open(os.path.join(sd, name)))
        args = [meta['privkey'], os.path.join(sd, meta['input']), meta.get('domain', 'zikaron/1')]
        a = run(RUST, ['sign'] + args); b = run(PY, ['sign'] + args)
        summary['sign'] += 1
        if a != b:
            divergences.append(('sign', name, a, b)); summary['sign_div'] += 1
        else:
            pred = meta.get('predicted')
            if pred and a[0]=='OK':
                try:
                    got=json.loads(a[1])
                    if got.get('sig')!=pred.get('sig'): summary['sign_vs_prediction_diff'] += 1
                except Exception: summary['sign_unparsable'] += 1
compare('audit', lambda p: [p], 'audit', exts=['.json'])

print('=== summary'); 
for k in sorted(summary): print(k, summary[k])
print('=== divergences', len(divergences))
groups = collections.defaultdict(list)
for cmd, name, a, b in divergences:
    key = (cmd, a[0], a[1][:120], b[0], b[1][:120])
    groups[key].append(name)
for key, names in sorted(groups.items(), key=lambda kv: -len(kv[1])):
    cmd, sa, oa, sb, ob = key
    print('\n--- %s x%d e.g. %s' % (cmd, len(names), ', '.join(names[:4])))
    print('  rust:', sa, oa)
    print('  py  :', sb, ob)
    for n in names[:2]:
        if n in manifest:
            print('  note:', {k: v for k, v in manifest[n].items() if k in ('note', 'predicted', 'predicted_label', 'category')})
print('=== agreed-but-differs-from-prediction', len(predmiss))
for x in predmiss[:40]: print('  ', x)
json.dump([{'cmd': c, 'file': n, 'rust': [a[0], a[1].decode('utf-8', 'replace')], 'py': [b[0], b[1].decode('utf-8', 'replace')]} for c, n, a, b in divergences],
          open(os.path.join(ROOT, 'divergences.json'), 'w'), indent=1)
