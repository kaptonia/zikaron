#!/usr/bin/env python3
"""Arc-level branch witnessing of a criterion program over its corpus (a measure,
never a criterion; see BOUNDARY-AUDIT.md).
Usage: bcov.py core|kit|scan <out.json>
Records every (prev_line -> line) arc executed inside the criterion files, then
lists every branch point (if/elif, while, for, try-handler) whose arms were not
both taken. Tooling only; the criterion is never modified."""
import sys, os, io, ast, json, collections, importlib, contextlib
ROOT = os.path.dirname(os.path.abspath(__file__))
mode, out = sys.argv[1], os.path.abspath(sys.argv[2])
if mode == 'core':
    FILES = [os.path.join(ROOT, 'impl-py', f) for f in ('zk1.py','zkcanon.py','zkcrypto.py','zkentry.py','zkaudit.py')]
elif mode == 'kit':
    FILES = [os.path.join(ROOT, 'kit-py', f) for f in ('zkk.py','zkkdoc.py','zkkkit.py','zkkread.py')] + \
            [os.path.join(ROOT, 'impl-py', f) for f in ('zkcanon.py','zkcrypto.py','zkentry.py','zkaudit.py')]
else:
    FILES = [os.path.join(ROOT, 'scan-py', 'scan_replay.py')]
TARGET = set(FILES)
ARCS = collections.defaultdict(set); PREV = {}
def tracer(frame, event, arg):
    f = frame.f_code.co_filename
    if f not in TARGET: return None
    k = id(frame)
    if event == 'call': PREV[k] = frame.f_lineno; ARCS[f].add((0, frame.f_lineno))
    elif event == 'line': ARCS[f].add((PREV.get(k, 0), frame.f_lineno)); PREV[k] = frame.f_lineno
    elif event == 'return': ARCS[f].add((PREV.get(k, 0), -1)); PREV.pop(k, None)
    elif event == 'exception': ARCS[f].add((frame.f_lineno, -2)); PREV[k] = frame.f_lineno
    return tracer
def run_main(main, argv):
    buf = io.TextIOWrapper(io.BytesIO()); err = io.StringIO()
    with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(err):
        try: main(argv)
        except SystemExit: pass
        except RecursionError: pass
sys.setrecursionlimit(10000)
cases = 0
if mode == 'core':
    sys.path.insert(0, os.path.join(ROOT, 'impl-py')); import zk1
    C = os.path.join(ROOT, 'corpus')
    def files(sub):
        d = os.path.join(C, sub)
        return [os.path.join(d, n) for n in sorted(os.listdir(d)) if os.path.isfile(os.path.join(d, n)) and not n.endswith(('.note', '.md'))]
    sys.settrace(tracer)
    for p in files('canon'):
        run_main(zk1.main, ['zk1', 'check', p]); run_main(zk1.main, ['zk1', 'canon', p]); cases += 2
    sd = os.path.join(C, 'sign')
    for n in sorted(os.listdir(sd)):
        if n.endswith('.meta.json'):
            m = json.load(open(os.path.join(sd, n)))
            run_main(zk1.main, ['zk1', 'sign', m['privkey'], os.path.join(sd, m['input']), m.get('domain', 'zikaron/1')]); cases += 1
    for p in files('audit'):
        if p.endswith('.json'): run_main(zk1.main, ['zk1', 'audit', p]); cases += 1
    sys.settrace(None)
elif mode == 'kit':
    sys.path.insert(0, os.path.join(ROOT, 'kit-py')); import zkk
    C = os.path.join(ROOT, 'kit-corpus'); os.chdir(C)
    m = json.load(open('manifest.json'))
    sys.settrace(tracer)
    for c in m['cases']:
        run_main(zkk.main, ['zkk'] + c['args']); cases += 1
    sys.settrace(None)
else:
    sys.path.insert(0, os.path.join(ROOT, 'scan-py')); import scan_replay
    F = os.path.normpath(os.path.join(ROOT, '..', 'zikaron-core', 'fixtures'))
    sys.settrace(tracer)
    for n in sorted(os.listdir(F)):
        if n.endswith('.json'):
            run_main(scan_replay.main, ['scan_replay', os.path.join(F, n)]); cases += 1
    sys.settrace(None)
# --- analysis
def lines_of(nodes):
    s = set()
    for n in nodes:
        for x in ast.walk(n):
            if hasattr(x, 'lineno'): s.update(range(x.lineno, (x.end_lineno or x.lineno) + 1))
    return s
misses = []; total = 0
for f in FILES:
    src = open(f, encoding='utf-8').read().splitlines(); tree = ast.parse('\n'.join(src)); arcs = ARCS.get(f, set())
    executed = {b for a, b in arcs} | {a for a, b in arcs}
    def add(kind, line, arm, ok):
        global total
        total += 1
        if not ok: misses.append({'file': os.path.relpath(f, ROOT), 'line': line, 'kind': kind, 'arm': arm, 'src': src[line-1].strip()[:110]})
    for node in ast.walk(tree):
        if isinstance(node, (ast.If, ast.While)):
            test = set(range(node.test.lineno, node.test.end_lineno + 1)) | {node.lineno}
            body = lines_of(node.body); orelse = lines_of(node.orelse)
            outs = {b for a, b in arcs if a in test and b not in test}
            if not outs and node.lineno not in executed:
                add(type(node).__name__, node.lineno, 'reached', False); continue
            add(type(node).__name__, node.lineno, 'true', bool(outs & body))
            if orelse: add(type(node).__name__, node.lineno, 'false', bool(outs & orelse))
            else: add(type(node).__name__, node.lineno, 'false', bool(outs - body))
        elif isinstance(node, ast.For):
            head = set(range(node.lineno, node.iter.end_lineno + 1)); body = lines_of(node.body)
            outs = {b for a, b in arcs if a in head and b not in head}
            if not outs and node.lineno not in executed:
                add('For', node.lineno, 'reached', False); continue
            add('For', node.lineno, 'enter', bool(outs & body)); add('For', node.lineno, 'exhaust', bool(outs - body))
        elif isinstance(node, ast.Try):
            for h in node.handlers:
                hl = lines_of([h])
                add('Except', h.lineno, 'caught', h.lineno in executed or bool(hl & executed))
        elif isinstance(node, ast.FunctionDef):
            add('Def', node.lineno, 'called', bool([b for a, b in arcs if a == 0 and b in lines_of([node])]))
json.dump({'mode': mode, 'cases': cases, 'branch_arms': total, 'misses': misses}, open(out, 'w'), indent=1)
print(mode, 'cases', cases, 'arms', total, 'missed', len(misses))
for m in misses: print('  %s:%d %s/%s  %s' % (m['file'], m['line'], m['kind'], m['arm'], m['src']))
