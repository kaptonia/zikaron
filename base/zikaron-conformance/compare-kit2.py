#!/usr/bin/env python3
"""Run a candidate zkk2 (ZKK2_CANDIDATE) and the zikaron.kit/2 criterion over
the kit-corpus2 manifest and report every divergence and every disagreement
with the generator's prediction.  The manifest is the corpus: every case it
names must exist and carry a prediction, the prediction is compared whole and
strictly (a bool is never an int), the manifest's counts must match its
cases, every file of the corpus tree is a case's path or argument, a hop
file a chain names, or lies under a kit's or a package's own directory, and
the run prints a digest over every corpus byte and case it read, the manifest
whole included, which ZKK2_EXPECT may pin.  A misuse case predicts {"exit": 2}: both programs exit 2 with
no output.  Exits 1 on any divergence or disagreement, 0 otherwise.  Run from
anywhere; paths in the manifest are relative to the corpus root; ZKK2_OUT
names the divergence file (default kit-divergences2.json beside this script).
The criterion runs with -B and writes no bytecode into the tree."""
import json, os, subprocess, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.join(HERE, 'kit-corpus2')
cand = os.environ.get('ZKK2_CANDIDATE') or sys.exit('set ZKK2_CANDIDATE to the candidate zkk2 command (HARNESS-KIT2.md contract)')
CAND = cand.split()
PY = [sys.executable, '-B', os.path.join(HERE, 'kit2-py', 'zkk2.py')]
OUT = os.environ.get('ZKK2_OUT') or os.path.join(HERE, 'kit-divergences2.json')
DIRS = ('badges', 'kits', 'packages', 'checks', 'chains')


READ = hashlib.sha256()   # a digest over every corpus byte the measurement reads (D5)
ALLOWED = {
    'badges': {'path', 'category', 'cmd', 'args', 'note', 'predicted', 'entry_paths'},
    'kits': {'path', 'category', 'cmd', 'args', 'note', 'predicted'},
    'packages': {'path', 'category', 'cmd', 'args', 'note', 'predicted'},
    'checks': {'path', 'category', 'cmd', 'args', 'note', 'predicted', 'audit_path', 'notices_path', 'now'},
    'chains': {'path', 'category', 'cmd', 'args', 'note', 'predicted', 'now'},
    'misuse': {'path', 'category', 'cmd', 'args', 'note', 'predicted'},
}


def inside(rel):
    full = os.path.normpath(os.path.join(CORPUS, rel))
    if not (full == CORPUS or full.startswith(CORPUS + os.sep)):
        return False
    cur = CORPUS
    for part in os.path.relpath(full, CORPUS).split(os.sep):
        cur = os.path.join(cur, part)
        if os.path.islink(cur) and part != os.path.relpath(full, CORPUS).split(os.sep)[-1]:
            return False
    return True


def digest_path(rel):
    """Fold a case's file, or every regular file under its directory, into READ."""
    full = os.path.join(CORPUS, rel)
    if os.path.isdir(full) and not os.path.islink(full):
        for dp, dn, fn in os.walk(full):
            for n in sorted(fn):
                p = os.path.join(dp, n)
                if os.path.isfile(p) and not os.path.islink(p):
                    try:
                        b = open(p, 'rb').read()
                    except OSError:
                        b = b'<unreadable>'
                    READ.update(os.path.relpath(p, CORPUS).encode() + b'\0' + b + b'\0')
    elif os.path.isfile(full):
        try:
            b = open(full, 'rb').read()
        except OSError:
            b = b'<unreadable>'
        READ.update(rel.encode() + b'\0' + b + b'\0')


def run(cmd, args):
    try:
        p = subprocess.run(cmd + args, capture_output=True, timeout=180, cwd=CORPUS)
    except subprocess.TimeoutExpired:
        return ('TIMEOUT', b'')
    if p.returncode != 0:
        return ('EXIT%d' % p.returncode, p.stdout[:300])
    return ('OK', p.stdout)


def tally_of(cat, c, got):
    """Fold one criterion answer into the tallies the manifest's coverage publishes (D4, D6)."""
    if not isinstance(got, dict):
        return
    if cat == 'badges':
        if c.get('cmd') == 'badge-decode':
            TALLY['badge_decode']['BADGE_OK' if got.get('ok') else got.get('token')] += 1
        else:
            TALLY['badge_encode']['payload' if 'payload' in got else got.get('token')] += 1
    elif cat == 'kits':
        TALLY['kit_verdicts'][got.get('verdict')] += 1
        if got.get('verdict') == 'E_KIT_MANIFEST':
            TALLY['kit_manifest_rules'][got.get('subject')] += 1
    elif cat == 'packages':
        TALLY['package_verdicts'][got.get('verdict')] += 1
    elif cat == 'checks':
        TALLY['check_verdicts'][got.get('verdict')] += 1
        for ch in got.get('checks') or []:
            if isinstance(ch, dict):
                TALLY['check_states']['%s:%s' % (ch.get('n'), ch.get('state'))] += 1
                if ch.get('reason'):
                    TALLY['check_reasons'][ch['reason']] += 1
    elif cat == 'chains':
        TALLY['chain_verdicts'][got.get('verdict')] += 1
        TALLY['chain_tokens'][str(got.get('token'))] += 1
        f = got.get('failing')
        TALLY['chain_failing_kinds'][f.get('kind') if isinstance(f, dict) else 'none'] += 1
        for lk in got.get('links') or []:
            TALLY['chain_links'][str(lk)] += 1


def same(a, b):
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    return a == b


try:
    MANIFEST_SHA = hashlib.sha256(open(os.path.join(CORPUS, 'manifest.json'), 'rb').read()).hexdigest()
    m = json.load(open(os.path.join(CORPUS, 'manifest.json')))
    cases = m['cases']
    assert isinstance(cases, list) and cases
except Exception as ex:
    print('manifest unreadable or empty:', ex); sys.exit(1)
print('=== manifest sha256 0x%s (seed %s)' % (MANIFEST_SHA, m.get('seed')))
READ.update(b'manifest.json\0' + open(os.path.join(CORPUS, 'manifest.json'), 'rb').read() + b'\0')   # the manifest whole is pinned (D3)
EXPECT = os.environ.get('ZKK2_EXPECT')
TALLY = collections.defaultdict(collections.Counter)
def TALLY_BY_KEY(key):
    return TALLY[key]
summary = collections.Counter(); div = []; predmiss = []
for c in cases:   # the measured case list is part of what EXPECT pins (D3)
    READ.update(json.dumps({k: c.get(k) for k in ('category', 'cmd', 'args', 'predicted')}, sort_keys=True, separators=(',', ':')).encode() + b'\0')

tally = collections.Counter(c.get('category') for c in cases)
for k, v in (m.get('counts') or {}).items():
    if tally.get(k, 0) != v:
        predmiss.append(('manifest', 'counts[%s]=%s' % (k, v), tally.get(k, 0), 'counts disagree with the cases'))
for k in tally:
    if k not in (m.get('counts') or {}) and k != 'misuse':
        predmiss.append(('manifest', k, None, 'counts lacks a category'))

# every file of the corpus tree, the generator, README and manifest apart, is a case's path or
# argument, lies under a case's directory, or is a hop file a chain case names
named = set()
for c in cases:
    for tok in [c.get('path', '')] + list(c.get('args', [])):
        if isinstance(tok, str) and '/' in tok:
            named.add(tok.rstrip('/'))
    if c.get('cmd') == 'chain-check':
        try:
            for h in (json.load(open(os.path.join(CORPUS, c['args'][1]))) if os.path.isfile(os.path.join(CORPUS, c['args'][1])) else []):   # a pipe is never opened (D4-4)
                for k in ('grant', 'audit', 'notices'):
                    if isinstance(h, dict) and isinstance(h.get(k), str):
                        named.add(h[k])
        except Exception:
            pass
subtree = set()   # a kit's or a package's own directory names its subtree; nothing else does
for c in cases:
    if c.get('category') in ('kits', 'packages') and isinstance(c.get('path'), str):
        subtree.add(c['path'].rstrip('/'))
def is_named(rel):
    if rel in named:
        return True
    parts = rel.split('/')
    for i in range(1, len(parts)):
        if '/'.join(parts[:i]) in subtree:
            return True
    return False
for rel in sorted(named):
    if not inside(rel):
        predmiss.append((rel, None, None, 'a case path leaves the corpus or crosses a symbolic link'))
for dirpath, dirnames, filenames in os.walk(CORPUS):
    dirnames[:] = [d for d in dirnames if d != '__pycache__']
    rel_dir = os.path.relpath(dirpath, CORPUS)
    if rel_dir != '.' and is_named(rel_dir):
        dirnames[:] = []
        continue
    for n in filenames + [d for d in dirnames if os.path.islink(os.path.join(dirpath, d))]:
        rel = os.path.normpath(os.path.join(rel_dir, n)) if rel_dir != '.' else n
        if rel in ('gen.py', 'README.md', 'manifest.json', '.gitignore'):
            continue
        if not is_named(rel):
            predmiss.append((rel, None, None, 'file no case names'))

for c in cases:
    cat = c.get('category')
    if cat not in ALLOWED:
        predmiss.append((c.get('path'), None, None, 'unknown category %r' % cat)); continue
    extra = set(c.keys()) - ALLOWED[cat]
    if extra:
        predmiss.append((c.get('path'), None, None, 'keys outside the generator\'s set: %s' % sorted(extra)))
    if 'predicted' not in c:
        predmiss.append((c.get('path'), 'no prediction', None, '')); continue
    args = c.get('args', [])
    if (args[0] if args else '') != c.get('cmd', ''):
        predmiss.append((c.get('path'), None, None, 'cmd differs from args[0]'))
    if cat in ('checks', 'chains'):
        ap = args[args.index('--audit') + 1] if '--audit' in args else None
        npth = args[args.index('--notices') + 1] if '--notices' in args else None
        nw = int(args[args.index('--now') + 1]) if '--now' in args and args[args.index('--now') + 1].isdigit() else None
        if cat == 'checks' and not same(c.get('audit_path'), ap):
            predmiss.append((c.get('path'), None, None, 'audit_path differs from args'))
        if cat == 'checks' and not same(c.get('notices_path'), npth):
            predmiss.append((c.get('path'), None, None, 'notices_path differs from args'))
        if not same(c.get('now'), nw):
            predmiss.append((c.get('path'), None, None, 'now differs from args'))
    if cat == 'badges' and 'entry_paths' in c and c.get('entry_paths') != args[1:]:
        predmiss.append((c.get('path'), None, None, 'entry_paths differs from args'))
    for t in args[1:]:
        if isinstance(t, str) and '/' in t and os.path.lexists(os.path.join(CORPUS, t)):
            digest_path(t)
    if cat == 'chains' and len(args) > 1:
        try:
            for h in (json.load(open(os.path.join(CORPUS, args[1]))) if os.path.isfile(os.path.join(CORPUS, args[1])) else []):   # a pipe is never opened (D4-4)
                for k in ('grant', 'audit', 'notices'):
                    if isinstance(h, dict) and isinstance(h.get(k), str) and os.path.lexists(os.path.join(CORPUS, h[k])):
                        digest_path(h[k])
        except Exception:
            pass
    if cat != 'misuse':
        first = next((t for t in args if isinstance(t, str) and '/' in t), None)
        if first is not None and not os.path.lexists(os.path.join(CORPUS, first)):
            predmiss.append((c.get('path'), 'file absent', None, first)); continue
    a = run(CAND, c['args']); b = run(PY, c['args'])
    summary[c['cmd']] += 1
    if a != b:
        div.append((c, a, b)); summary[c['cmd'] + '_div'] += 1
        if b[0] == 'OK':
            try:
                tally_of(cat, c, json.loads(b[1]))
            except Exception:
                pass
        continue
    pred = c['predicted']
    if c.get('category') == 'misuse':
        if not same(pred, {'exit': 2}):
            predmiss.append((c['path'], pred, None, 'a misuse case predicts {"exit": 2}'))
        elif a[0] != 'EXIT2' or a[1] != b'':
            predmiss.append((c['path'], pred, a[0], 'misuse: expected exit 2 with no output'))
        continue
    if a[0] != 'OK':
        predmiss.append((c['path'], pred, a[0], 'no answer')); continue
    try:
        got = json.loads(a[1])
    except Exception:
        predmiss.append((c['path'], pred, None, 'unparsable')); continue
    if not same(pred, got):
        predmiss.append((c['path'], pred, got, c.get('note', '')[:80]))
    tally_of(cat, c, got)
cov = m.get('coverage') or {}
COV_KEYS = ('badge_decode', 'badge_encode', 'kit_verdicts', 'kit_manifest_rules', 'package_verdicts',
            'check_states', 'check_reasons', 'check_verdicts', 'chain_verdicts', 'chain_tokens',
            'chain_links', 'chain_failing_kinds')
if not isinstance(cov, dict) or set(cov.keys()) != set(COV_KEYS):
    predmiss.append(('manifest', 'coverage', None, 'coverage keys are not the twelve the generator publishes'))
for key in COV_KEYS:
    pub = cov.get(key) if isinstance(cov, dict) else None
    mine = TALLY[key]
    if not isinstance(pub, dict) or dict(pub) != {k: v for k, v in mine.items() if v}:
        predmiss.append(('manifest', key, None, 'coverage differs from the criterion\'s measured tally'))
# the residue of the closed vocabularies is the published unwitnessed list (D4, D3): the comparator
# carries the closed vocabularies itself, as a table of [coverage key, residue label, token] drawn from
# the law's closed sets; the manifest's `vocabularies` table must equal it, and a label is unwitnessed
# iff the criterion's measured tally under its key lacks its token
VOCAB = [
    ["badge_decode", "badge:E_BADGE_PREFIX", "E_BADGE_PREFIX"],
    ["badge_decode", "badge:E_BADGE_CAP", "E_BADGE_CAP"],
    ["badge_decode", "badge:E_BADGE_B64", "E_BADGE_B64"],
    ["badge_decode", "badge:E_BADGE_ENTRY", "E_BADGE_ENTRY"],
    ["badge_decode", "badge:E_BADGE_TYPE", "E_BADGE_TYPE"],
    ["badge_decode", "badge:E_BADGE_INCOMPLETE", "E_BADGE_INCOMPLETE"],
    ["badge_decode", "badge:E_BADGE_LINK", "E_BADGE_LINK"],
    ["badge_decode", "badge:BADGE_OK", "BADGE_OK"],
    ["kit_verdicts", "kit:KIT_OK", "KIT_OK"],
    ["kit_verdicts", "kit:E_KIT_UNREADABLE", "E_KIT_UNREADABLE"],
    ["kit_verdicts", "kit:E_KIT_MANIFEST_ABSENT", "E_KIT_MANIFEST_ABSENT"],
    ["kit_verdicts", "kit:E_KIT_MANIFEST", "E_KIT_MANIFEST"],
    ["kit_verdicts", "kit:E_KIT_ENTRY_BYTES", "E_KIT_ENTRY_BYTES"],
    ["kit_verdicts", "kit:E_KIT_FILE", "E_KIT_FILE"],
    ["kit_verdicts", "kit:E_KIT_CONTENT", "E_KIT_CONTENT"],
    ["kit_verdicts", "kit:E_KIT_EXTRA", "E_KIT_EXTRA"],
    ["kit_manifest_rules", "kit-rule:canonical", "canonical"],
    ["kit_manifest_rules", "kit-rule:members", "members"],
    ["kit_manifest_rules", "kit-rule:spec", "spec"],
    ["kit_manifest_rules", "kit-rule:root", "root"],
    ["kit_manifest_rules", "kit-rule:entries", "entries"],
    ["kit_manifest_rules", "kit-rule:files", "files"],
    ["kit_manifest_rules", "kit-rule:contents", "contents"],
    ["kit_manifest_rules", "kit-rule:anchoredOn", "anchoredOn"],
    ["kit_manifest_rules", "kit-rule:anchors", "anchors"],
    ["kit_manifest_rules", "kit-rule:note_md", "note_md"],
    ["package_verdicts", "package:GP_OK", "GP_OK"],
    ["package_verdicts", "package:E_GP_NO_CODE", "E_GP_NO_CODE"],
    ["package_verdicts", "package:E_GP_ROOT", "E_GP_ROOT"],
    ["package_verdicts", "package:E_GP_UNCARRIED", "E_GP_UNCARRIED"],
    ["package_verdicts", "package:E_GP_CONTENT", "E_GP_CONTENT"],
    ["package_verdicts", "package:E_GP_TERMS", "E_GP_TERMS"],
    ["package_verdicts", "package:E_KIT_MANIFEST", "E_KIT_MANIFEST"],
    ["package_verdicts", "package:E_KIT_UNREADABLE", "E_KIT_UNREADABLE"],
    ["package_verdicts", "package:E_BADGE_ENTRY", "E_BADGE_ENTRY"],
    ["check_states", "check1:PASS", "1:PASS"],
    ["check_states", "check1:FAIL", "1:FAIL"],
    ["check_states", "check2:PASS", "2:PASS"],
    ["check_states", "check2:FAIL", "2:FAIL"],
    ["check_states", "check2:UNKNOWN", "2:UNKNOWN"],
    ["check_states", "check3:PASS", "3:PASS"],
    ["check_states", "check3:FAIL", "3:FAIL"],
    ["check_states", "check3:UNKNOWN", "3:UNKNOWN"],
    ["check_states", "check4:PASS", "4:PASS"],
    ["check_states", "check4:FAIL", "4:FAIL"],
    ["check_states", "check4:UNKNOWN", "4:UNKNOWN"],
    ["check_states", "check5:PASS", "5:PASS"],
    ["check_states", "check5:FAIL", "5:FAIL"],
    ["check_states", "check5:UNKNOWN", "5:UNKNOWN"],
    ["check_states", "check6:PASS", "6:PASS"],
    ["check_states", "check6:FAIL", "6:FAIL"],
    ["check_states", "check6:UNKNOWN", "6:UNKNOWN"],
    ["check_verdicts", "grant-check:GREEN", "GREEN"],
    ["check_verdicts", "grant-check:PARTIAL", "PARTIAL"],
    ["check_verdicts", "grant-check:FAIL", "FAIL"],
    ["chain_verdicts", "chain-check:GREEN", "GREEN"],
    ["chain_verdicts", "chain-check:PARTIAL", "PARTIAL"],
    ["chain_verdicts", "chain-check:FAIL", "FAIL"],
    ["chain_tokens", "chain-token:CHAIN_EMPTY", "CHAIN_EMPTY"],
    ["chain_tokens", "chain-token:CHAIN_INCOMPLETE", "CHAIN_INCOMPLETE"],
    ["chain_tokens", "chain-token:CHAIN_LINK", "CHAIN_LINK"],
    ["chain_tokens", "chain-token:None", "None"],
    ["chain_links", "chain-link:True", "True"],
    ["chain_links", "chain-link:False", "False"],
    ["chain_links", "chain-link:None", "None"],
    ["chain_failing_kinds", "chain-failing:empty", "empty"],
    ["chain_failing_kinds", "chain-failing:incomplete", "incomplete"],
    ["chain_failing_kinds", "chain-failing:link", "link"],
    ["chain_failing_kinds", "chain-failing:hop", "hop"],
    ["chain_failing_kinds", "chain-failing:none", "none"],
    ["check_reasons", "check1-reason:NOT_A_GRANT", "NOT_A_GRANT"],
    ["check_reasons", "check1-reason:E_UTF8", "E_UTF8"],
    ["check_reasons", "check1-reason:E_JSON", "E_JSON"],
    ["check_reasons", "check1-reason:E_NUMBER", "E_NUMBER"],
    ["check_reasons", "check1-reason:E_DEPTH", "E_DEPTH"],
    ["check_reasons", "check1-reason:E_DUP_KEY", "E_DUP_KEY"],
    ["check_reasons", "check1-reason:E_KEY_CHARSET", "E_KEY_CHARSET"],
    ["check_reasons", "check1-reason:E_VALUE_CHARSET", "E_VALUE_CHARSET"],
    ["check_reasons", "check1-reason:E_NOT_CANONICAL", "E_NOT_CANONICAL"],
    ["check_reasons", "check1-reason:E_ENVELOPE", "E_ENVELOPE"],
    ["check_reasons", "check1-reason:E_ENVELOPE_MISSING", "E_ENVELOPE_MISSING"],
    ["check_reasons", "check1-reason:E_ENVELOPE_CLOSED", "E_ENVELOPE_CLOSED"],
    ["check_reasons", "check1-reason:E_SPEC", "E_SPEC"],
    ["check_reasons", "check1-reason:E_ENTRYTYPE", "E_ENTRYTYPE"],
    ["check_reasons", "check1-reason:E_AUTHOR", "E_AUTHOR"],
    ["check_reasons", "check1-reason:E_SEQ", "E_SEQ"],
    ["check_reasons", "check1-reason:E_PREV", "E_PREV"],
    ["check_reasons", "check1-reason:E_PREV_SEQ", "E_PREV_SEQ"],
    ["check_reasons", "check1-reason:E_BODY", "E_BODY"],
    ["check_reasons", "check1-reason:E_SIG_FORM", "E_SIG_FORM"],
    ["check_reasons", "check1-reason:E_GENESIS_PLACE", "E_GENESIS_PLACE"],
    ["check_reasons", "check1-reason:E_BODY_FIELD", "E_BODY_FIELD"],
    ["check_reasons", "check1-reason:E_SIG_V", "E_SIG_V"],
    ["check_reasons", "check1-reason:E_SIG_RANGE", "E_SIG_RANGE"],
    ["check_reasons", "check1-reason:E_SIG_HIGH_S", "E_SIG_HIGH_S"],
    ["check_reasons", "check1-reason:E_SIG_RECOVER", "E_SIG_RECOVER"],
    ["check_reasons", "check1-reason:E_SIG_SIGNER", "E_SIG_SIGNER"],
]
if m.get('vocabularies') != VOCAB:
    predmiss.append(('manifest', 'vocabularies', None, 'the manifest\'s vocabularies table is not the comparator\'s'))
residue = sorted(set(lab for key, lab, tok in VOCAB if tok not in TALLY_BY_KEY(key)))
if residue != sorted(m.get('unwitnessed') or []):
    predmiss.append(('manifest', 'vocabularies', None, 'unwitnessed is not the closed vocabularies minus the measured tally'))
if m.get('undecided'):
    predmiss.append(('manifest', 'vocabularies', None, 'undecided is not empty'))
print('=== summary')
for k in sorted(summary): print(' ', k or '(no command)', summary[k])
print('=== bytes read sha256 0x%s' % READ.hexdigest())
if EXPECT is not None and EXPECT.lower() != '0x' + READ.hexdigest():
    predmiss.append(('corpus', EXPECT, '0x' + READ.hexdigest(), 'bytes read digest is not the expected one'))
print('=== divergences', len(div))
groups = collections.defaultdict(list)
for c, a, b in div:
    groups[(c['cmd'], a[0], a[1][:160], b[0], b[1][:160])].append(c['path'])
for key, paths in sorted(groups.items(), key=lambda kv: -len(kv[1])):
    print('\n--- %s x%d e.g. %s' % (key[0], len(paths), ', '.join(paths[:3])))
    print('  candidate:', key[1], key[2]); print('  criterion:', key[3], key[4])
print('=== disagreements with prediction', len(predmiss))
for x in predmiss[:40]: print('  ', str(x)[:400])
json.dump([{'case': c['path'], 'candidate': [a[0], a[1].decode('utf-8', 'replace')], 'criterion': [b[0], b[1].decode('utf-8', 'replace')]} for c, a, b in div], open(OUT, 'w'), indent=1)
sys.exit(1 if (div or predmiss) else 0)
