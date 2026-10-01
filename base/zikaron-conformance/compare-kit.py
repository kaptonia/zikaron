#!/usr/bin/env python3
"""Run a candidate zkk (ZKK_CANDIDATE) and the kit criterion over the kit corpus
and report every divergence, and every disagreement with the generator's prediction. Run from anywhere; paths in the manifest are
relative to the corpus root."""
import json, os, subprocess, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
CORPUS = os.path.join(HERE, 'kit-corpus')
RUST = [os.environ.get('ZKK_CANDIDATE') or sys.exit('set ZKK_CANDIDATE to the candidate zkk binary (HARNESS-KIT.md contract)')]
PY = ['python3', os.path.join(HERE, 'kit-py', 'zkk.py')]

def run(cmd, args):
    try:
        p = subprocess.run(cmd + args, capture_output=True, timeout=120, cwd=CORPUS)
    except subprocess.TimeoutExpired:
        return ('TIMEOUT', b'')
    if p.returncode != 0:
        # the diagnostic on stderr is outside the contract (HARNESS.md); only
        # the exit code and stdout are compared
        return ('EXIT%d' % p.returncode, p.stdout[:300])
    return ('OK', p.stdout)

m = json.load(open(os.path.join(CORPUS, 'manifest.json')))
cases = m['cases']
summary = collections.Counter(); div = []; predmiss = []
for c in cases:
    a = run(RUST, c['args']); b = run(PY, c['args'])
    summary[c['cmd']] += 1
    if a != b:
        div.append((c, a, b)); summary[c['cmd'] + '_div'] += 1
        continue
    pred = c.get('predicted')
    if pred and a[0] == 'OK':
        try:
            got = json.loads(a[1])
        except Exception:
            summary['unparsable'] += 1; continue
        # third-opinion comparison on the whole predicted object
        if pred != got:
            predmiss.append((c['path'], pred, got, c.get('note', '')[:80]))
print('=== summary')
for k in sorted(summary): print(' ', k, summary[k])
print('=== divergences', len(div))
groups = collections.defaultdict(list)
for c, a, b in div:
    groups[(c['cmd'], a[0], a[1][:160], b[0], b[1][:160])].append(c['path'])
for key, paths in sorted(groups.items(), key=lambda kv: -len(kv[1])):
    print('\n--- %s x%d e.g. %s' % (key[0], len(paths), ', '.join(paths[:3])))
    print('  rust:', key[1], key[2]); print('  py  :', key[3], key[4])
print('=== agreed but differs from prediction', len(predmiss))
for x in predmiss[:40]: print('  ', x)
json.dump([{'case': c['path'], 'rust': [a[0], a[1].decode('utf-8', 'replace')], 'py': [b[0], b[1].decode('utf-8', 'replace')]} for c, a, b in div], open(os.path.join(HERE, 'kit-divergences.json'), 'w'), indent=1)
