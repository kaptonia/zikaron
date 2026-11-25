#!/usr/bin/env python3
"""Run a candidate zk2 (ZK2_CANDIDATE) and the zikaron/2 criterion over the
corpus2 manifest and report every divergence and every disagreement with the
generator's prediction.  The manifest is the corpus: every case it names must
exist, a file no case names is a disagreement, the manifest's counts must
match its cases, and each category's prediction keys are required exactly.
Values are compared strictly (a bool is never an int, an int never a float).
Exits 1 on any divergence or disagreement, 0 otherwise.  Run from anywhere;
set ZK2_CANDIDATE to the candidate's zk2 command (a path, or a command line
split on spaces); ZK2_OUT names the divergence file (default
divergences2.json beside this script).  The criterion runs with -B and
writes no bytecode into the tree."""
import json, os, subprocess, sys, collections, hashlib
MANIFEST_SHA = None
ROOT = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.join(ROOT, 'corpus2')
cand = os.environ.get('ZK2_CANDIDATE') or sys.exit('set ZK2_CANDIDATE to the candidate zk2 command (HARNESS2.md contract)')
CAND = cand.split()
PY = [sys.executable, '-B', os.path.join(ROOT, 'impl2-py', 'zk2.py')]
OUT = os.environ.get('ZK2_OUT') or os.path.join(ROOT, 'divergences2.json')

COUNT_KEYS = ("entries", "findings", "hard", "f_SEQ_GAP", "f_PREV_MISMATCH", "f_AUTHORITY_MISMATCH",
              "f_ROOT_MISMATCH", "f_EQUIVOCATION", "missing", "anchored", "unanchored", "excluded",
              "retired", "unknown_type", "malformed", "unproven", "void", "discarded", "continues")
LIST_KEYS = ("missing", "anchored", "unanchored", "excluded", "retired", "unknown_type",
             "malformed", "unproven", "void", "discarded")
OLD_LIST_KEYS = ("missing", "anchored", "unanchored", "excluded", "unknown_type", "malformed",
                 "unproven", "void", "discarded", "unavailable", "adoption_unproven")
ALLOWED = {
    'canon': {'path', 'bytes', 'category', 'note', 'predicted', 'predicted_entry_id', 'predicted_canon',
              'predicted_canon_sha256', 'hand_expect', 'disagreement'},
    'sign': {'path', 'meta', 'category', 'note', 'privkey', 'domain', 'predicted'},
    'audit': {'path', 'note_file', 'category', 'note', 'predicted_label', 'predicted_counts',
              'predicted_old', 'predicted_continues', 'hand_expect_label', 'disagreement'},
    'misuse': {'path', 'category', 'args', 'note', 'predicted'},
}
REQUIRED = {'canon': {'path', 'predicted'}, 'sign': {'path', 'meta', 'predicted'},
            'audit': {'path', 'note_file', 'predicted_label'}, 'misuse': {'path', 'args', 'predicted'}}


READ = hashlib.sha256()   # a digest over every corpus byte the measurement reads (D5)


def read_bytes(path):
    b = open(path, 'rb').read()
    READ.update(path[len(CORPUS):].encode('utf-8', 'surrogateescape') + b'\0' + b + b'\0')
    return b


def inside(rel):
    """A corpus-relative path that stays inside the corpus and crosses no symbolic link."""
    full = os.path.normpath(os.path.join(CORPUS, rel))
    if not (full == CORPUS or full.startswith(CORPUS + os.sep)):
        return False
    cur = CORPUS
    for part in os.path.relpath(full, CORPUS).split(os.sep):
        cur = os.path.join(cur, part)
        if os.path.islink(cur):
            return False
    return True


def run(cmd, args, timeout=120):
    try:
        p = subprocess.run(cmd + args, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return ('TIMEOUT', b'')
    if p.returncode != 0:
        return ('EXIT%d' % p.returncode, p.stdout[:400])
    return ('OK', p.stdout)


def same(a, b):
    """Strict structural equality: types must agree (bool is not int, int is
    not float), objects key for key, arrays element for element."""
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    return a == b


summary = collections.Counter()
divergences = []
disagreements = []


def disagree(name, what):
    summary['disagreement'] += 1
    disagreements.append((name, what))


try:
    MANIFEST_SHA = hashlib.sha256(open(os.path.join(CORPUS, 'manifest.json'), 'rb').read()).hexdigest()
    man = json.load(open(os.path.join(CORPUS, 'manifest.json')))
    cases = man['cases']
    assert isinstance(cases, list) and cases
except Exception as ex:
    print('manifest unreadable or empty:', ex); sys.exit(1)
print('=== manifest sha256 0x%s (seed %s)' % (MANIFEST_SHA, man.get('seed')))
READ.update(b'manifest.json\0' + open(os.path.join(CORPUS, 'manifest.json'), 'rb').read() + b'\0')   # the manifest whole is pinned (D3)
EXPECT = os.environ.get('ZK2_EXPECT')   # the expected bytes-read digest, 0x-prefixed; exit 1 on any other (D5)
TALLY = {'tokens': collections.Counter(), 'labels': collections.Counter(), 'rows': collections.Counter(), 'verdicts': collections.Counter()}
def TALLY_BY_KEY(key):
    return {'sec_10_tokens_and_accept': TALLY['tokens'], 'sec_8_6_labels': TALLY['labels'],
            'sec_8_6_rows_and_findings': TALLY['rows'], 'sec_9_3_anchor_verdicts': TALLY['verdicts']}.get(key, {})
for c in man.get('cases') or []:   # the measured case list is part of what EXPECT pins (D3)
    READ.update(json.dumps({k: c.get(k) for k in ('category', 'predicted', 'predicted_label', 'predicted_counts', 'predicted_old', 'predicted_continues', 'args')}, sort_keys=True, separators=(',', ':')).encode() + b'\0')

for c in cases:
    cat = c.get('category')
    if cat not in ALLOWED:
        disagree(c.get('path', '?'), 'unknown category %r' % cat); continue
    extra = set(c.keys()) - ALLOWED[cat]
    missing = REQUIRED[cat] - set(c.keys())
    if extra:
        disagree(c.get('path'), 'keys outside the generator\'s set for %s: %s' % (cat, sorted(extra)))
    if missing:
        disagree(c.get('path'), 'keys missing for %s: %s' % (cat, sorted(missing)))
    # the hand label and the disagreement flag agree with the prediction (D7)
    hand = c.get('hand_expect_label') if cat == 'audit' else c.get('hand_expect')
    pred = c.get('predicted_label') if cat == 'audit' else c.get('predicted')
    if hand is not None and (hand != pred) != bool(c.get('disagreement')):
        disagree(c.get('path'), 'disagreement flag does not match the hand label against the prediction')
    if c.get('disagreement'):
        disagree(c.get('path'), 'the corpus carries a case whose hand label disagrees with its prediction')

# the manifest's counts are the cases it holds
tally = collections.Counter(c.get('category') for c in cases)
for k, v in (man.get('counts') or {}).items():
    if tally.get(k, 0) != v:
        disagree('manifest', 'counts[%s] says %s, cases hold %s' % (k, v, tally.get(k, 0)))
for k in tally:
    if k not in (man.get('counts') or {}) and k != 'misuse':
        disagree('manifest', 'counts lacks category %s' % k)

# every file of the corpus tree, the generator, README and manifest apart, is named by a case
named = set()
for c in cases:
    if c.get('category') == 'misuse':
        for tok in c.get('args', [])[1:]:
            if isinstance(tok, str) and '/' in tok:
                named.add(tok)
        continue
    named.add(c['path'])
    for k in ('note_file', 'meta'):
        if k in c:
            named.add(c[k])
for rel in sorted(named):
    if not inside(rel):
        disagree(rel, 'a case path leaves the corpus or crosses a symbolic link')
for dirpath, dirnames, filenames in os.walk(CORPUS):
    dirnames[:] = [d for d in dirnames if d != '__pycache__']
    for d in list(dirnames):
        if os.path.islink(os.path.join(dirpath, d)):
            disagree(os.path.relpath(os.path.join(dirpath, d), CORPUS), 'a symbolic link in the corpus')
    for n in filenames:
        full = os.path.join(dirpath, n)
        rel = os.path.relpath(full, CORPUS)
        if os.path.islink(full):
            disagree(rel, 'a symbolic link in the corpus'); continue
        if rel in ('gen.py', 'README.md', 'manifest.json'):
            continue
        if rel not in named:
            disagree(rel, 'file no case names')


def parse(a):
    try:
        return json.loads(a[1])
    except Exception:
        return None


def corpus_path(rel):
    return os.path.join(CORPUS, rel)


for c in cases:
    cat = c.get('category')
    if cat == 'misuse':
        # the command word stays as written; a later token containing '/' that
        # names an existing corpus path is passed as that path
        args = []
        for i, tok in enumerate(c.get('args', [])):
            if i > 0 and isinstance(tok, str) and '/' in tok and os.path.lexists(corpus_path(tok)):
                if not inside(tok):
                    disagree(c['path'], 'a misuse token leaves the corpus'); break
                if os.path.isfile(corpus_path(tok)):
                    read_bytes(corpus_path(tok))
                args.append(corpus_path(tok))
            else:
                args.append(tok)
        a = run(CAND, args); b = run(PY, args)
        summary['misuse'] += 1
        if a != b:
            divergences.append(('misuse', c['path'], a, b)); summary['misuse_div'] += 1; continue
        if not same(c.get('predicted'), {'exit': 2}):
            disagree(c['path'], 'a misuse case predicts {"exit": 2}'); continue
        if a[0] != 'EXIT2' or a[1] != b'':
            disagree(c['path'], 'misuse: expected exit 2 with no output, got %s %r' % (a[0], a[1][:40]))
        continue
    path = corpus_path(c['path'])
    if not inside(c['path']) or not os.path.exists(path):
        disagree(c['path'], 'file absent'); continue
    read_bytes(path)
    for k in ('note_file', 'meta'):
        if k in c and (not inside(c[k]) or not os.path.exists(corpus_path(c[k]))):
            disagree(c['path'], '%s absent' % k)
        elif k in c:
            read_bytes(corpus_path(c[k]))
    if cat == 'canon':
        for cmd in ('check', 'canon'):
            a = run(CAND, [cmd, path]); b = run(PY, [cmd, path])
            summary[cmd] += 1
            if a != b:
                divergences.append((cmd, c['path'], a, b)); summary[cmd + '_div'] += 1; continue
            got = parse(a)
            if a[0] != 'OK' or got is None:
                disagree(c['path'], '%s: no answer %r' % (cmd, a[0])); continue
            if cmd == 'check':
                if 'predicted' not in c:
                    disagree(c['path'], 'no predicted'); continue
                got_t = 'ok' if got.get('ok') is True else got.get('token')
                TALLY['tokens'][got_t] += 1
                if c['predicted'] != got_t:
                    disagree(c['path'], 'check predicted %s got %s' % (c['predicted'], got_t))
                elif got.get('ok') is True and not same(c.get('predicted_entry_id'), got.get('entry_id')):
                    disagree(c['path'], 'entry_id differs from prediction')
            else:
                pc = c.get('predicted_canon')
                if pc is None:
                    disagree(c['path'], 'no predicted_canon'); continue
                if pc == 'ok':
                    if got.get('ok') is not True:
                        disagree(c['path'], 'canon predicted ok got %s' % got.get('token'))
                    else:
                        h = hashlib.sha256(bytes.fromhex(got['canon'][2:])).hexdigest()
                        if '0x' + h != c.get('predicted_canon_sha256'):
                            disagree(c['path'], 'canonical bytes differ from prediction')
                elif got.get('ok') is True or got.get('token') != pc:
                    disagree(c['path'], 'canon predicted %s got %s' % (pc, got.get('token')))
    elif cat == 'sign':
        if 'meta' not in c or not os.path.exists(corpus_path(c['meta'])):
            disagree(c['path'], 'sign: no meta'); continue
        meta = json.load(open(corpus_path(c['meta'])))
        if meta.get('privkey') != c.get('privkey') or ('domain' in meta) != ('domain' in c) \
                or meta.get('domain') != c.get('domain') or not same(meta.get('predicted'), c.get('predicted')) \
                or meta.get('input') != os.path.basename(c['path']):
            disagree(c['path'], 'sign: the meta and the manifest disagree'); continue
        args = [meta['privkey'], os.path.join(CORPUS, 'sign', meta['input'])]
        if 'domain' in meta:
            args.append(meta['domain'])
        a = run(CAND, ['sign'] + args); b = run(PY, ['sign'] + args)
        summary['sign'] += 1
        if a != b:
            divergences.append(('sign', c['path'], a, b)); summary['sign_div'] += 1; continue
        got = parse(a)
        pred = c.get('predicted') or {}
        if a[0] != 'OK' or got is None or not pred:
            disagree(c['path'], 'sign: no answer or no prediction'); continue
        if set(pred.keys()) != {'digest', 'presig', 'sig', 'signer'}:
            disagree(c['path'], 'sign: prediction keys are digest, presig, sig, signer'); continue
        for k in ('digest', 'presig', 'sig', 'signer'):
            if not same(got.get(k), pred.get(k)):
                disagree(c['path'], 'sign %s differs from prediction' % k); break
    elif cat == 'audit':
        try:
            _doc = json.loads(open(os.path.join(CORPUS, c['path']), 'rb').read())
            for _a in (_doc.get('anchors') if isinstance(_doc, dict) else None) or []:
                if isinstance(_a, dict) and _a.get('verdict') in ('counted', 'UNPROVEN', 'VOID'):
                    TALLY['verdicts'][_a['verdict']] += 1
        except Exception:
            pass
        a = run(CAND, ['audit', path]); b = run(PY, ['audit', path])
        summary['audit'] += 1
        if a != b:
            divergences.append(('audit', c['path'], a, b)); summary['audit_div'] += 1; continue
        got = parse(a)
        pred = c.get('predicted_label')
        if a[0] != 'OK' or got is None or pred is None:
            disagree(c['path'], 'audit: no answer or no prediction'); continue
        if pred == 'NO_LABEL':
            if not same(got, {'ok': False, 'reason': 'NO_LABEL'}):
                disagree(c['path'], 'predicted NO_LABEL got %s' % str(got)[:80])
            else:
                TALLY['labels']['NO_LABEL'] += 1
            continue
        if got.get('label') != pred:
            disagree(c['path'], 'label predicted %s got %s' % (pred, got.get('label'))); continue
        TALLY['labels'][got['label']] += 1
        for f in got.get('findings', []):
            TALLY['rows'][f.get('name')] += 1
        for k in LIST_KEYS + ('continues',):
            v = got.get(k)
            if (isinstance(v, list) and v) or (k == 'continues' and v is not None):
                TALLY['rows'][k.upper()] += (len(v) if isinstance(v, list) else 1)
        pc = c.get('predicted_continues')
        if (pc is None) != (got.get('continues') is None):
            disagree(c['path'], 'predicted_continues present iff continues is'); continue
        if pc is not None:
            nested = got['continues']
            if not isinstance(pc, dict) or set(pc.keys()) != {'entry_id', 'cut'}:
                disagree(c['path'], 'predicted_continues is exactly {entry_id, cut}'); continue
            if not isinstance(nested, dict) or not same(pc.get('entry_id'), nested.get('entry_id')) or not same(pc.get('cut'), nested.get('cut')):
                disagree(c['path'], 'the selected continuation differs from the prediction'); continue
        counts = c.get('predicted_counts') or {}
        if set(counts.keys()) != set(COUNT_KEYS):
            disagree(c['path'], 'predicted_counts keys must be exactly the generator\'s %d keys' % len(COUNT_KEYS)); continue
        for k in COUNT_KEYS:
            v = counts[k]
            if type(v) is not int:
                disagree(c['path'], '%s predicted is not an int' % k); break
            if k == 'entries':
                if not same(got.get('entries'), v):
                    disagree(c['path'], 'entries predicted %s got %s' % (v, got.get('entries'))); break
            elif k in LIST_KEYS:
                if not isinstance(got.get(k), list) or len(got[k]) != v:
                    disagree(c['path'], '%s predicted %s got %s' % (k, v, len(got[k]) if isinstance(got.get(k), list) else 'absent')); break
            elif k == 'findings':
                if not isinstance(got.get('findings'), list) or len(got['findings']) != v:
                    disagree(c['path'], 'findings predicted %s got %s' % (v, len(got.get('findings', [])))); break
            elif k == 'hard':
                if sum(1 for f in got.get('findings', []) if f.get('hard') is True) != v:
                    disagree(c['path'], 'hard count differs'); break
            elif k.startswith('f_'):
                if sum(1 for f in got.get('findings', []) if f.get('name') == k[2:]) != v:
                    disagree(c['path'], '%s differs' % k); break
            elif k == 'continues':
                if (0 if got.get('continues') is None else 1) != v:
                    disagree(c['path'], 'continues presence differs'); break
        else:
            # the nested zikaron/1 report of a continued ledger
            old = c.get('predicted_old')
            nested = got.get('continues')
            if (old is None) != (nested is None):
                disagree(c['path'], 'predicted_old present iff continues is'); continue
            if old is not None:
                rep = nested.get('report') if isinstance(nested, dict) else None
                if not isinstance(rep, dict):
                    disagree(c['path'], 'continues.report absent'); continue
                if set(old.keys()) != set(('label', 'entries', 'findings') + OLD_LIST_KEYS) \
                        or not all(type(old[k]) is int for k in old if k != 'label'):
                    disagree(c['path'], 'predicted_old keys must be exactly the generator\'s, every count an int'); continue
                if rep.get('label') != old.get('label'):
                    disagree(c['path'], 'old label predicted %s got %s' % (old.get('label'), rep.get('label'))); continue
                if not same(rep.get('entries'), old.get('entries')):
                    disagree(c['path'], 'old entries differ'); continue
                for k in OLD_LIST_KEYS:
                    if k in old and (not isinstance(rep.get(k), list) or len(rep[k]) != old[k]):
                        disagree(c['path'], 'old %s predicted %s got %s' % (k, old[k], len(rep[k]) if isinstance(rep.get(k), list) else 'absent')); break
                if 'findings' in old and len(rep.get('findings', [])) != old['findings']:
                    disagree(c['path'], 'old findings differ')
    else:
        pass

# the published coverage is the criterion's measured tally (D6)
cov = man.get('coverage') or {}
if not isinstance(cov, dict) or set(cov.keys()) != {'sec_10_tokens_and_accept', 'sec_8_6_labels', 'sec_8_6_rows_and_findings', 'sec_9_3_anchor_verdicts'}:
    disagree('manifest', 'coverage keys are not the four the generator publishes')
for key, mine in (('sec_10_tokens_and_accept', TALLY['tokens']), ('sec_8_6_labels', TALLY['labels']),
                  ('sec_8_6_rows_and_findings', TALLY['rows']), ('sec_9_3_anchor_verdicts', TALLY['verdicts'])):
    pub = cov.get(key) if isinstance(cov, dict) else None
    if not isinstance(pub, dict) or {k: v for k, v in pub.items()} != {k: v for k, v in mine.items() if v}:
        disagree('manifest', 'coverage[%s] differs from the criterion\'s measured tally' % key)
# the residue of the closed vocabularies is the published unwitnessed list (D4, D3): the comparator
# carries the closed vocabularies itself, as a table of [coverage key, residue label, token] drawn from
# the law's closed sets; the manifest's `vocabularies` table must equal it, and a label is unwitnessed
# iff the criterion's measured tally under its key lacks its token
VOCAB = [
    ["sec_10_tokens_and_accept", "E_UTF8", "E_UTF8"],
    ["sec_10_tokens_and_accept", "E_JSON", "E_JSON"],
    ["sec_10_tokens_and_accept", "E_NUMBER", "E_NUMBER"],
    ["sec_10_tokens_and_accept", "E_DEPTH", "E_DEPTH"],
    ["sec_10_tokens_and_accept", "E_DUP_KEY", "E_DUP_KEY"],
    ["sec_10_tokens_and_accept", "E_KEY_CHARSET", "E_KEY_CHARSET"],
    ["sec_10_tokens_and_accept", "E_VALUE_CHARSET", "E_VALUE_CHARSET"],
    ["sec_10_tokens_and_accept", "E_NOT_CANONICAL", "E_NOT_CANONICAL"],
    ["sec_10_tokens_and_accept", "E_ENVELOPE", "E_ENVELOPE"],
    ["sec_10_tokens_and_accept", "E_ENVELOPE_MISSING", "E_ENVELOPE_MISSING"],
    ["sec_10_tokens_and_accept", "E_ENVELOPE_CLOSED", "E_ENVELOPE_CLOSED"],
    ["sec_10_tokens_and_accept", "E_SPEC", "E_SPEC"],
    ["sec_10_tokens_and_accept", "E_ENTRYTYPE", "E_ENTRYTYPE"],
    ["sec_10_tokens_and_accept", "E_AUTHOR", "E_AUTHOR"],
    ["sec_10_tokens_and_accept", "E_SEQ", "E_SEQ"],
    ["sec_10_tokens_and_accept", "E_PREV", "E_PREV"],
    ["sec_10_tokens_and_accept", "E_PREV_SEQ", "E_PREV_SEQ"],
    ["sec_10_tokens_and_accept", "E_BODY", "E_BODY"],
    ["sec_10_tokens_and_accept", "E_SIG_FORM", "E_SIG_FORM"],
    ["sec_10_tokens_and_accept", "E_GENESIS_PLACE", "E_GENESIS_PLACE"],
    ["sec_10_tokens_and_accept", "E_BODY_FIELD", "E_BODY_FIELD"],
    ["sec_10_tokens_and_accept", "E_SIG_V", "E_SIG_V"],
    ["sec_10_tokens_and_accept", "E_SIG_RANGE", "E_SIG_RANGE"],
    ["sec_10_tokens_and_accept", "E_SIG_HIGH_S", "E_SIG_HIGH_S"],
    ["sec_10_tokens_and_accept", "E_SIG_RECOVER", "E_SIG_RECOVER"],
    ["sec_10_tokens_and_accept", "E_SIG_SIGNER", "E_SIG_SIGNER"],
    ["sec_10_tokens_and_accept", "ok", "ok"],
    ["sec_8_6_rows_and_findings", "SEQ_GAP", "SEQ_GAP"],
    ["sec_8_6_rows_and_findings", "PREV_MISMATCH", "PREV_MISMATCH"],
    ["sec_8_6_rows_and_findings", "AUTHORITY_MISMATCH", "AUTHORITY_MISMATCH"],
    ["sec_8_6_rows_and_findings", "ROOT_MISMATCH", "ROOT_MISMATCH"],
    ["sec_8_6_rows_and_findings", "EQUIVOCATION", "EQUIVOCATION"],
    ["sec_8_6_rows_and_findings", "MISSING", "MISSING"],
    ["sec_8_6_rows_and_findings", "UNANCHORED", "UNANCHORED"],
    ["sec_8_6_rows_and_findings", "EXCLUDED", "EXCLUDED"],
    ["sec_8_6_rows_and_findings", "RETIRED", "RETIRED"],
    ["sec_8_6_rows_and_findings", "CONTINUES", "CONTINUES"],
    ["sec_8_6_rows_and_findings", "ANCHORED", "ANCHORED"],
    ["sec_8_6_rows_and_findings", "DISCARDED", "DISCARDED"],
    ["sec_8_6_rows_and_findings", "MALFORMED", "MALFORMED"],
    ["sec_8_6_rows_and_findings", "UNKNOWN_TYPE", "UNKNOWN_TYPE"],
    ["sec_8_6_rows_and_findings", "UNPROVEN", "UNPROVEN"],
    ["sec_8_6_rows_and_findings", "VOID", "VOID"],
    ["sec_8_6_labels", "BROKEN_CHAIN", "BROKEN_CHAIN"],
    ["sec_8_6_labels", "UNPROVEN", "UNPROVEN"],
    ["sec_8_6_labels", "GAPS", "GAPS"],
    ["sec_8_6_labels", "COMPLETE", "COMPLETE"],
    ["sec_8_6_labels", "NO_LABEL", "NO_LABEL"],
    ["sec_9_3_anchor_verdicts", "counted", "counted"],
    ["sec_9_3_anchor_verdicts", "UNPROVEN", "UNPROVEN"],
    ["sec_9_3_anchor_verdicts", "VOID", "VOID"],
]
if man.get('vocabularies') != VOCAB:
    disagree('manifest', 'the manifest\'s vocabularies table is not the comparator\'s')
residue = sorted(set(lab for key, lab, tok in VOCAB if tok not in TALLY_BY_KEY(key)))
if residue != sorted(man.get('unwitnessed') or []):
    disagree('manifest', 'unwitnessed is not the closed vocabularies minus the measured tally')
if man.get('undecided'):
    disagree('manifest', 'undecided is not empty')
print('=== summary')
for k in sorted(summary): print(k, summary[k])
print('=== bytes read sha256 0x%s' % READ.hexdigest())
if EXPECT is not None and EXPECT.lower() != '0x' + READ.hexdigest():
    disagree('corpus', 'bytes read digest 0x%s is not the expected %s' % (READ.hexdigest(), EXPECT))
print('=== divergences', len(divergences))
groups = collections.defaultdict(list)
for cmd, name, a, b in divergences:
    groups[(cmd, a[0], a[1][:120], b[0], b[1][:120])].append(name)
for key, names in sorted(groups.items(), key=lambda kv: -len(kv[1])):
    cmd, sa, oa, sb, ob = key
    print('\n--- %s x%d e.g. %s' % (cmd, len(names), ', '.join(names[:4])))
    print('  candidate:', sa, oa)
    print('  criterion:', sb, ob)
print('=== disagreements with prediction', len(disagreements))
for x in disagreements[:40]: print('  ', x)
json.dump([{'cmd': c, 'file': n, 'candidate': [a[0], a[1].decode('utf-8', 'replace')], 'criterion': [b[0], b[1].decode('utf-8', 'replace')]} for c, n, a, b in divergences],
          open(OUT, 'w'), indent=1)
sys.exit(1 if (divergences or disagreements) else 0)
