#!/usr/bin/env python3
"""Run a candidate zkf (ZKF_CANDIDATE) and the zikaron.face/1 criterion over
the face-corpus manifest and report every divergence and every disagreement
with the generator's prediction.  The manifest is the corpus: every case it
names must exist and carry a prediction, the prediction is the exit code and
the exact bytes of stdout, the manifest's counts must match its cases, every
file of the corpus tree is a case's path, and the run prints the manifest's
sha256 and a digest over every corpus byte and case it read, the manifest
whole included, which ZKF_EXPECT may pin.  A misuse case predicts
{"exit": 2}: both programs exit 2 with no output.  The labels of the closed
vocabularies are tallied from the criterion's own answers and compared with
the manifest's coverage; a label nothing witnesses is a disagreement.  Exits
1 on any divergence or disagreement, 0 otherwise.  Run from anywhere; paths
in the manifest are relative to the corpus root; ZKF_OUT names the
divergence file (default face-divergences.json beside this script).  The
manifest's boundary list (law sec 10.1) must equal the comparator's own, and the
criterion's answer of every case it names must pass the test of its side.  The
criterion runs with -B and writes no bytecode into the tree."""
import collections, hashlib, json, os, re, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.join(HERE, 'face-corpus')
cand = os.environ.get('ZKF_CANDIDATE') or sys.exit('set ZKF_CANDIDATE to the candidate zkf command (HARNESS-FACE.md contract)')
CAND = cand.split()
PY = [sys.executable, '-B', os.path.join(HERE, 'face-py', 'zkf.py')]
OUT = os.environ.get('ZKF_OUT') or os.path.join(HERE, 'face-divergences.json')
EXPECT = os.environ.get('ZKF_EXPECT')
DIRS = ('entries', 'carried', 'tables', 'joined')
CATEGORIES = {'entries': 'entry', 'declare': 'declare', 'carried': 'carried', 'tables': 'table', 'joined': 'joined', 'misuse': None}
CASE_KEYS = {'path', 'category', 'cmd', 'args', 'note', 'predicted'}
SHAPES = ('home', 'account', 'key', 'persona', 'release', 'reroot', 'origin')
REASONS = ('SHAPE', 'ROOT', 'AUTHOR', 'SIGNATURE', 'SUBJECT', 'UNSEATED')
# the closed vocabularies of the law (sec 8) and of the harness's answers, carried here and not read
# from the manifest: the manifest's `vocabularies` must equal this list
VOCAB = (['entry:rejected', 'entry:other-type', 'shape:none', 'rooted:true', 'rooted:false',
          'consent:true', 'consent:false', 'declare:plain', 'declare:signed',
          'carried:entry', 'carried:null', 'table:NO_TABLE', 'table:read', 'home:null', 'home:set',
          'lineage:root-alone', 'lineage:moved', 'homes:none', 'homes:one', 'homes:several',
          'span:open', 'span:closed', 'unread:zero', 'unread:nonzero', 'unproven:zero',
          'unproven:nonzero', 'persona:null', 'persona:set', 'misuse']
         + ['shape:' + s for s in SHAPES] + ['state:bound', 'state:released']
         + ['reason:' + r for r in REASONS]
         + ['reroot:null', 'reroot:set', 'origin:null', 'origin:set',
            'joined:true', 'joined:false', 'joined:NO_TABLE'])

READ = hashlib.sha256()

# ---- the boundary list (law sec 10.1), carried here with the test of each side; the manifest's
# `boundaries` must name the same boundaries and cases, and each named case's criterion answer must
# pass its side's test.  A side is a list of (case slug, test over the parsed answer, None for exit 2).
def _depth(x):
    if not isinstance(x, (dict, list)): return 0
    return 1 + max([_depth(y) for y in (x.values() if isinstance(x, dict) else x)] or [0])
def ent(shape='any'):
    def f(a):
        if not a or not a.get('ok') or not a.get('face'): return False
        return a['shape'] is not None if shape == 'any' else a['shape'] == shape
    return f
noshape = lambda a: bool(a) and a.get('ok') and a.get('face') and a['shape'] is None
rej = lambda a: bool(a) and a.get('ok') is False and 'token' in a
cons = lambda v: (lambda a: bool(a) and a.get('consent') is v)
rooted = lambda v: (lambda a: bool(a) and a.get('rooted') is v)
other = lambda a: bool(a) and a.get('ok') and a.get('face') is False
car = lambda a: bool(a) and a.get('carried') is not None
nocar = lambda a: bool(a) and 'carried' in a and a['carried'] is None
tab = lambda a: bool(a) and 'faces' in a
notab = lambda a: bool(a) and a.get('reason') == 'NO_TABLE'
dec = lambda a: bool(a) and 'digest' in a
mis = lambda a: a is None
def reason(r): return lambda a: tab(a) and any(m['reason'] == r for m in a['misread'])
def T(pred): return lambda a: tab(a) and pred(a)
class I(object):
    """A side test that reads the case's input file beside its answer, for a tie no answer shows alone."""
    def __init__(self, f): self.f = f
def bound(a, handle): return any(r.get('handle') == handle and r['state'] == 'bound' for r in a['faces'])
def absent(a, *handles): return not any(r.get('handle') in handles for r in a['faces'])
def reasons(a): return sorted(m['reason'] for m in a['misread'])
def span0(a): return a['faces'][0]['spans'][0]
def rows_ordered(a):
    ks = [r['kind'] for r in a['faces']]
    acc = [(r['platform'].encode(), r['handle'].encode()) for r in a['faces'] if r['kind'] == 'account']
    key = [(r['address'].encode(), r['role'].encode()) for r in a['faces'] if r['kind'] == 'key']
    return ks == sorted(ks) and len(acc) == 6 and len(key) == 4 and acc == sorted(set(acc)) and key == sorted(set(key))
def least_tx(block):
    return I(lambda a, v: tab(a) and span0(a)['since']['tx'] == min(r['tx'] for r in v['anchors'] if r['blockNumber'] == block and r['hash'] == span0(a)['entry']))
def least_release(block):
    return I(lambda a, v: tab(a) and span0(a)['release'] == min(r['hash'] for r in v['anchors'] if r['blockNumber'] == block))
B = [
 # sec 1 labels and handles
 ('1 label length 1..32', [('platform_one', ent()), ('platform_len32', ent())], [('platform_empty', noshape), ('platform_len33', noshape)]),
 ('1 label alphabet a-z', [('platform_mixed', ent()), ('platform_z', ent())], [('platform_upper', noshape), ('platform_backtick', noshape), ('platform_brace', noshape)]),
 ('1 label alphabet 0-9', [('platform_digit', ent()), ('platform_nine', ent())], [('platform_slash', noshape), ('platform_colon', noshape)]),
 ('1 label hyphen only', [('platform_hyphen', ent())], [('platform_dot', noshape), ('platform_underscore', noshape)]),
 ('1 role label (key row)', [('role_len32', ent()), ('role_edges', ent())], [('role_len33', noshape), ('role_colon', noshape)]),
 ('1 label as declare argument', [('role_len32', dec), ('role_edges', dec)], [('declare_role_len33', mis), ('declare_role_colon', mis), ('declare_role_comma', mis)]),
 ('1 handle length 1..128', [('handle_one', ent()), ('handle_len128', ent())], [('handle_empty', noshape), ('handle_len129', noshape)]),
 ('1 handle ends not U+0020', [('handle_inner_space', ent())], [('handle_lead_space', noshape), ('handle_trail_space', noshape), ('handle_only_space', noshape)]),
 ('1 handle within U+0020..U+007E (parent)', [('handle_printable', ent())], [('handle_non_ascii', rej), ('handle_tab', rej)]),
 ('1 face entry type literal', [('home', ent())], [('type_next', other), ('type_longer', other), ('type_cased', other)]),
 # sec 2.2 shapes
 ('2.2 home exact members', [('home', ent('home'))], [('home_without_chain', noshape), ('home_extra_member', noshape), ('home_with_string_note', noshape)]),
 ('2.2 home chain int', [('home_chain_zero', ent('home')), ('home_chain_max', ent('home'))], [('home_chain_string', noshape), ('home_chain_bool', noshape)]),
 ('2.2 account optional note', [('account', ent('account')), ('account_note', ent('account')), ('account_note_empty', ent('account'))], [('account_note_int', noshape), ('account_note_null', noshape)]),
 ('2.2 account exact members', [('account', ent('account'))], [('account_without_handle', noshape), ('account_extra_member', noshape), ('account_with_address', noshape)]),
 ('2.2 key exact members', [('key', ent('key')), ('key_note', ent('key'))], [('key_without_sig', noshape), ('key_extra_member', noshape), ('key_with_handle', noshape)]),
 ('2.2 release exact members', [('release', ent('release')), ('release_note', ent('release'))], [('release_without_subject', noshape), ('release_extra_member', noshape), ('release_with_kind', noshape)]),
 ('2.2 act closed', [('account', ent('account'))], [('act_unknown', noshape), ('act_cased', noshape), ('act_space', noshape)]),
 ('2.2 kind closed', [('key', ent('key'))], [('kind_unknown', noshape), ('kind_cased', noshape), ('kind_key_over_account_members', noshape)]),
 ('2.2 root hex20', [('home', ent())], [('home_root_upper', noshape), ('account_root_short', noshape), ('key_root_bare', noshape), ('release_root_hex32', noshape)]),
 ('2.2 address hex20', [('key', ent('key'))], [('address_upper', noshape), ('address_short', noshape)]),
 ('2.2 sig hex65', [('key', ent('key'))], [('sig_short', noshape), ('sig_long', noshape), ('sig_upper', noshape)]),
 ('2.2 subject hex32', [('release', ent('release'))], [('subject_short', noshape), ('subject_hex20', noshape), ('subject_upper', noshape)]),
 ('entry rooted', [('home', rooted(True))], [('unrooted_home', rooted(False))]),
 # sec 3
 ('3.1 declaration exact', [('key', cons(True))], [('consent_trailing_lf', cons(False)), ('consent_crlf', cons(False)), ('consent_upper_hex', cons(False)), ('consent_no_0x', cons(False)), ('consent_lines_swapped', cons(False))]),
 ('3.2 domain', [('key', cons(True))], [('consent_entry_domain', cons(False)), ('consent_next_domain', cons(False)), ('consent_personal_sign_of_text', cons(False)), ('consent_hash_not_hex', cons(False))]),
 ('3.3 v in {27,28}', [('key', cons(True))], [('consent_v_26', cons(False)), ('consent_v_29', cons(False)), ('consent_v_recid', cons(False))]),
 ('3.3 r,s in [1,n-1]', [('key', cons(True))], [('consent_r_zero', cons(False)), ('consent_s_zero', cons(False)), ('consent_r_is_n', cons(False)), ('consent_s_is_n', cons(False))]),
 ('3.3 s <= (n-1)/2', [('consent_s_at_half', lambda a: bool(a) and a.get('shape') == 'key')], [('consent_s_half_plus_one', cons(False)), ('consent_high_s', cons(False))]),
 ('3.3 recovery and address', [('key', cons(True)), ('key_face_is_root', cons(True))], [('consent_r_off_curve', cons(False)), ('consent_other_key', cons(False)), ('consent_v_flipped', cons(False))]),
 ('3.3 own three members', [('unrooted_key_consent_holds', cons(True))], [('consent_other_root', cons(False)), ('consent_other_role', cons(False)), ('consent_other_address', cons(False))]),
 # sec 5.1
 ('5.1 length > 36', [('len37', nocar), ('account', car)], [('len36', nocar), ('len35', nocar)]),
 ('5.1 selector', [('account', car)], [('selector_last_byte', nocar), ('selector_zero', nocar), ('anchor_many', nocar)]),
 ('5.1 sha256(tail) = c[4:36]', [('account', car)], [('id_bit_flipped', nocar), ('tail_byte_flipped', nocar), ('id_of_other_entry', nocar)]),
 ('5.1 accept(tail)', [('account', car), ('history', car), ('shapeless_face_entry', car)], [('tail_not_entry', nocar), ('tail_v1_entry', nocar), ('tail_plus_lf_new_id', nocar)]),
 # sec 6.1
 ('6.1 five members', [('empty', tab)], [('invalid_without_root', notab), ('invalid_without_pile', notab), ('invalid_extra_member', notab)]),
 ('6.1 root hex20', [('empty', tab)], [('invalid_root_upper', notab), ('invalid_root_short', notab)]),
 ('6.1 chain six members', [('empty', tab)], [('invalid_chain_without_book', notab), ('invalid_chain_extra_member', notab)]),
 ('6.1 chain ints', [('empty', tab)], [('invalid_chain_id_string', notab), ('invalid_chain_from_string', notab), ('invalid_chain_to_string', notab), ('invalid_chain_book_start_string', notab), ('invalid_chain_from_false', notab), ('invalid_chain_id_string_alone', notab)]),
 ('6.1 fromBlock <= toBlock', [('valid_from_equals_to', tab)], [('invalid_from_above_to', notab), ('invalid_from_above_to_alone', notab)]),
 ('6.1 registries hex20 strictly ascending', [('valid_registries_two', tab), ('valid_registries_empty', tab)], [('invalid_registries_descending', notab), ('invalid_registries_unsorted', notab), ('invalid_registries_upper', notab), ('invalid_chain_registry_int', notab)]),
 ('6.1 book null iff bookFromBlock null', [('home_without_book', tab), ('empty', tab)], [('invalid_book_without_start', notab), ('invalid_start_without_book', notab)]),
 ('6.1 book hex20', [('empty', tab)], [('invalid_book_upper', notab), ('invalid_chain_book_int', notab)]),
 ('6.1 bookFromBlock <= toBlock', [('valid_book_start_equals_to', tab)], [('invalid_book_start_above_to', notab), ('invalid_book_start_above_to_alone', notab)]),
 ('6.1 chains strictly ascending', [('empty', tab)], [('invalid_chains_descending', notab), ('invalid_chains_repeated', notab)]),
 ('6.1 anchor seven members and forms', [('home_then_account', tab), ('valid_anchor_extra_member', T(lambda a: bound(a, 'alice'))), ('valid_anchor_three_extra_members', T(lambda a: bound(a, 'extra-members'))),
      ('valid_anchor_extra_members_of_every_kind', T(lambda a: bound(a, 'alice')))], [('invalid_anchor_without_hash', notab), ('invalid_anchor_extra_member_without_verdict', notab), ('invalid_anchor_tx_short', notab), ('invalid_anchor_sender_upper', notab), ('invalid_anchor_hash_short', notab), ('invalid_anchor_hash_upper', notab), ('invalid_anchor_time_string', notab), ('invalid_anchor_tx_upper', notab)]),
 ('6.1 verdict closed', [('verdicts', tab)], [('invalid_anchor_verdict', notab), ('invalid_anchor_verdict_null', notab)]),
 ('6.1 anchor chain declared, block in range', [('valid_anchor_at_from', tab), ('valid_anchor_at_to', tab)], [('invalid_anchor_chain_undeclared', notab), ('invalid_anchor_below_range', notab), ('invalid_anchor_above_range_alone', notab)]),
 ('6.1 accession nine members and forms', [('home_book_alone', tab)], [('invalid_accession_without_root', notab), ('invalid_accession_root_upper', notab), ('invalid_accession_root_hex32', notab), ('invalid_accession_root_null', notab), ('invalid_accession_without_to', notab), ('invalid_accession_extra_member', notab), ('invalid_accession_holder_short', notab), ('invalid_accession_log_index_string', notab), ('invalid_accession_to_upper', notab), ('invalid_accession_tx_short', notab), ('invalid_accession_tx_index_string', notab), ('invalid_accession_time_string', notab), ('invalid_accession_tx_index_true', notab), ('invalid_accession_tx_upper', notab), ('invalid_accession_holder_upper', notab)]),
 ('6.1 accession on a booked chain, in book range', [('valid_accession_at_book_start', tab), ('valid_accession_at_to', tab), ('valid_accession_below_from_block', tab)], [('invalid_accession_no_book', notab), ('invalid_accession_below_book_start', notab), ('invalid_accession_chain_undeclared', notab), ('invalid_anchor_above_range', notab)]),
 ('6.1 pile spelling', [('valid_pile_upper_digits', tab), ('valid_pile_empty_string', tab)], [('invalid_pile_no_prefix', notab), ('invalid_pile_upper_prefix', notab), ('invalid_pile_odd', notab), ('invalid_pile_not_hex', notab), ('invalid_pile_int', notab)]),
 # sec 6.2
 ('6.2 anchor key (chain, block, tx, hash)', [('valid_anchor_same_tx_other_hash', tab), ('valid_anchor_same_hash_other_block', tab), ('valid_clock_other_chain_free', tab), ('valid_anchor_same_hash_other_tx', tab)], [('invalid_anchor_twice_other_verdict', notab), ('invalid_anchor_twice_other_sender', notab), ('invalid_anchor_twice_other_time', notab)]),
 ('6.2 two anchor records that agree on the seven are one record',
     [('valid_anchor_twice', tab),
      ('valid_anchor_twice_unproven_once', T(lambda a: a['unproven'] == 1)),
      ('valid_anchor_twice_opening_once', T(lambda a: a['home'] == 1 and len(a['homes']) == 1)),
      ('valid_anchor_twice_extra_member_differs', T(lambda a: a['unproven'] == 1))], []),
 ('6.2 accession key (chain, block, txIndex, logIndex)', [('valid_accession_other_log_index', tab), ('valid_accession_other_tx_index', tab), ('valid_accession_same_position_other_block', tab), ('valid_accession_same_position_other_chain', tab)], [('invalid_accession_twice', notab), ('invalid_accession_same_position', notab), ('invalid_accession_same_position_other_tx', notab), ('invalid_accession_same_position_other_holder', notab), ('invalid_accession_same_position_other_root', notab)]),
 ('6.2 one time per block', [('valid_clock_stands_still', tab)], [('invalid_clock_two_times_one_block', notab), ('invalid_clock_anchor_and_accession', notab), ('invalid_clock_two_accessions_one_block', notab)]),
 ('6.2 clock monotone per chain', [('valid_clock_stands_still', tab), ('valid_clock_other_chain_free', tab)], [('invalid_clock_runs_back', notab), ('invalid_clock_accession_runs_back', notab), ('invalid_clock_anchor_runs_back_after_accession', notab)]),
 ('transport reader: no byte order mark', [('spelling_pretty', tab)], [('file_bom', notab)]),
 ('transport reader: integers within [0, 2^53 - 1]', [('int_max', tab)], [('file_int_2p53', notab)]),
 ('transport reader: no member named twice', [('spelling_pretty', tab)], [('file_duplicate_member', notab)]),
 ('transport reader: scalar values only', [('spelling_escapes', tab)], [('file_lone_surrogate_in_name', notab)]),
 ('transport reader: depth 128', [('anchor_member_depth_128', I(lambda a, v: tab(a) and bound(a, 'deep') and _depth(v) == 128))],
  [('file_depth_129', notab), ('anchor_member_depth_129', I(lambda a, v: notab(a) and _depth(v) == 129))]),
 # sec 7.1 order
 ('7.1 time before chainId', [('time_before_chain_id', T(lambda a: a['home'] == 1))], []),
 ('7.1 chainId at one second (both orders)', [('same_second_anchor_on_lower_chain', T(lambda a: a['home'] == 1)), ('same_second_anchor_on_higher_chain', T(lambda a: a['home'] is None))], []),
 ('7.1 block order at one second', [('blocks_sharing_a_second', T(lambda a: bound(a, 'first') and bound(a, 'second') and a['misread'] == []))], []),
 # sec 7.2
 ('7.2 anchors before accessions in a step', [('accession_block', T(lambda a: bound(a, 'old-in-block') and bound(a, 'new-after') and absent(a, 'new-in-block', 'old-after')))], []),
 ('7.2 accessions by txIndex then logIndex', [('two_accessions_one_block_in_order', T(lambda a: len(a['lineage']) == 3)), ('two_accessions_one_block_reversed', T(lambda a: len(a['lineage']) == 2 and a['office'] == a['lineage'][1]['key'])), ('two_accessions_same_holder', T(lambda a: len(a['lineage']) == 2 and a['lineage'][1]['accession']['logIndex'] == 4)), ('accessions_tx_index_first', T(lambda a: len(a['lineage']) == 3))], []),
 ('7.2 accession moves only from the office', [('two_accessions_one_block_in_order', T(lambda a: len(a['lineage']) == 3))], [('accession_by_non_office', T(lambda a: len(a['lineage']) == 1)),
     ('retired_root_hands_on_under_root', T(lambda a: len(a['lineage']) == 2 and a['home'] == 1 and len(a['homes']) == 1 and bound(a, 'seat-keeps') and a['misread'] == []))]),
 ('7.2 accession moves only under the root R',
     [('accession_enrolment_first_record', T(lambda a: len(a['lineage']) == 2 and a['home'] == 1 and bound(a, 'enrolled-first'))),
      ('accession_after_enrolment', T(lambda a: len(a['lineage']) == 4 and a['home'] == 10 and a['lineage'][3]['accession']['chainId'] == 10 and bound(a, 'k3-after-enrolment')))],
     [('accession_other_root_moves_nothing', T(lambda a: len(a['lineage']) == 1 and bound(a, 'root-still') and absent(a, 'k1-under-another-root') and a['misread'] == [])),
      ('accession_in_book_not_entered', T(lambda a: len(a['lineage']) == 2 and a['home'] == 1 and bound(a, 'k1-still-office') and absent(a, 'k2-unseated')))]),
 ('7.2 accession to itself lengthens the line', [('handover_to_itself', T(lambda a: len(a['lineage']) == 3))], []),
 ('7.2 home nothing: every book; home set: home book only', [('first_handover_wins', T(lambda a: len(a['lineage']) == 2 and a['home'] == 1 and len(a['homes']) == 1 and a['homes'][0]['sender'] == a['lineage'][1]['key'])), ('line_grows_by_time', T(lambda a: len(a['lineage']) == 3 and a['lineage'][2]['accession']['chainId'] == 10 and a['home'] is None))],
     [('home_book_alone', T(lambda a: len(a['lineage']) == 2 and a['lineage'][1]['accession']['chainId'] == 1 and bound(a, 'still-root') and bound(a, 'k2-home') and absent(a, 'k1-nowhere'))), ('home_without_book', T(lambda a: len(a['lineage']) == 1 and a['home'] == 1 and bound(a, 'alice')))]),
 ("7.2 opening needs office, counted, home shape, root R, chain = step, author the office, the book's head the office",
     [('home_then_account', T(lambda a: a['home'] == 1)), ('home_by_office_after_handover', T(lambda a: a['home'] == 1 and len(a['lineage']) == 2 and bound(a, 'alice'))),
      ('opening_after_entering_the_book', T(lambda a: a['home'] == 10 and len(a['lineage']) == 2 and bound(a, 'entered-then-opened')))],
     [('opening_void', T(lambda a: a['home'] is None)), ('retired_key_cannot_open', T(lambda a: a['home'] is None)), ('home_entries_that_open_nothing', T(lambda a: len(a['homes']) == 1 and a['home'] == 1 and bound(a, 'alice') and reasons(a) == ['ROOT', 'UNSEATED', 'UNSEATED'])), ('home_by_stranger_author', T(lambda a: a['home'] is None)), ('home_by_earlier_key', T(lambda a: a['home'] is None and a['homes'] == [])),
      ('opening_needs_the_books_head', T(lambda a: a['home'] == 1 and len(a['homes']) == 1 and bound(a, 'home-where-the-book-holds-it') and reasons(a) == ['UNSEATED'])),
      ('opening_in_the_block_of_entering', T(lambda a: a['home'] is None and a['homes'] == [] and reasons(a) == ['UNSEATED', 'UNSEATED'])),
      ('opening_without_book_by_successor', T(lambda a: a['home'] is None and a['homes'] == [] and reasons(a) == ['UNSEATED', 'UNSEATED'])),
      ('opening_where_the_book_moved_on', T(lambda a: a['home'] == 1 and len(a['homes']) == 1 and len(a['lineage']) == 2 and bound(a, 'seat-keeps') and bound(a, 'incoming-at-home') and absent(a, 'outgoing-on-the-other-chain'))),
      ('opening_where_the_book_names_another', T(lambda a: a['home'] is None and a['homes'] == [] and len(a['lineage']) == 2 and reasons(a) == ['UNSEATED', 'UNSEATED']))]),
 ('7.2 home from the next step', [('home_then_account', T(lambda a: a['home'] == 1 and bound(a, 'alice')))], [('bind_in_opening_block', reason('UNSEATED'))]),
 # sec 7.3
 ('7.3 seated only on the home', [('binding_on_two_chains', T(lambda a: span0(a)['since']['chainId'] == 1 and a['faces'][0]['state'] == 'bound'))], [('binding_off_home', reason('UNSEATED')), ('bind_without_home', reason('UNSEATED'))]),
 ('7.3 the author is the office as the step found it', [('author_office_after_handover', T(lambda a: bound(a, 'by-k1-in-office') and a['misread'] == []))],
     [('author_old_and_future', T(lambda a: a['faces'] == [] and reasons(a) == ['UNSEATED', 'UNSEATED'])),
      ('author_retired_office_anchors_binding', T(lambda a: a['faces'] == [] and reasons(a) == ['UNSEATED'])),
      ('author_retired_office_anchors_key_face', T(lambda a: a['faces'] == [] and reasons(a) == ['UNSEATED']))]),
 ('7.3 moment order: time, chain, block, tx', [('moments_are_earliest', T(lambda a: span0(a)['since']['blockNumber'] == 12)), ('moment_chain_before_block', T(lambda a: span0(a)['since']['chainId'] == 1)), ('moment_tx_tie_greater_listed_first', least_tx(11))], []),
 # sec 7.4, 7.5
 ('7.4 release stands only on a binding', [('bind_then_release', T(lambda a: a['faces'] and a['faces'][0]['state'] == 'released'))], [('release_of_no_binding', reason('SUBJECT')), ('release_of_release_and_stranger_release', reason('SUBJECT'))]),
 ('7.5 least release, entry tie', [('moments_are_earliest', T(lambda a: span0(a)['until']['blockNumber'] == 30)), ('two_releases_one_transaction', least_release(20))], []),
 ('7.5 release before binding clamps', [('release_before_binding', T(lambda a: span0(a)['release'] is not None and span0(a)['until'] == span0(a)['since'] and span0(a)['since']['blockNumber'] == 15))], []),
 ('7.5 state bound iff an open span', [('two_bindings_one_released', T(lambda a: a['faces'][0]['state'] == 'bound'))], [('bind_then_release', T(lambda a: a['faces'][0]['state'] == 'released'))]),
 ('7.8 spans and homes entry ties', [('two_bindings_one_transaction', T(lambda a: len(a['faces'][0]['spans']) == 2 and a['faces'][0]['spans'][0]['since'] == a['faces'][0]['spans'][1]['since'] and a['faces'][0]['spans'][0]['entry'] < a['faces'][0]['spans'][1]['entry'])), ('two_openings_one_transaction', T(lambda a: len(a['homes']) == 2 and a['homes'][0]['tx'] == a['homes'][1]['tx'] and a['homes'][0]['entry'] < a['homes'][1]['entry']))], []),
 ('7.8 row order accounts then keys', [('rows_in_order', T(rows_ordered))], []),
 # sec 7.6
 ('7.6 each reason', [('reasons_first_applies', reason('SHAPE')), ('reasons_first_applies', reason('ROOT')), ('strangers', reason('AUTHOR')), ('key_faces', reason('SIGNATURE')), ('release_of_no_binding', reason('SUBJECT')), ('bind_without_home', reason('UNSEATED'))], []),
 ('7.6 AUTHOR before SUBJECT', [('release_of_release_and_stranger_release', reason('AUTHOR'))], []),
 # sec 7.7
 ('7.7 unread distinct, in office, any chain, any type', [('unread', T(lambda a: a['unread'] == 2)), ('unread_before_home_by_step_office', T(lambda a: a['unread'] == 2))], [('empty', T(lambda a: a['unread'] == 0))]),
]

B += [
 ('7.6 listed when a key of the line anchored it, whatever the verdict',
     [('misread_scope', T(lambda a: [m['reason'] for m in a['misread']].count('UNSEATED') == 2)), ('reasons_first_applies', T(lambda a: len(a['misread']) == 5))],
     [('misread_scope_fork_key', T(lambda a: a['misread'] == [])), ('pile', T(lambda a: a['misread'] == [])), ('misread_scope', T(lambda a: len(a['misread']) == 3))]),
 ('7.7 unproven counts the office\'s UNPROVEN records',
     [('unproven_counts_the_office', T(lambda a: a['unproven'] == 2)), ('unproven_opening', T(lambda a: a['unproven'] == 1 and a['home'] is None)), ('unproven_release', T(lambda a: a['unproven'] == 1 and a['faces'][0]['state'] == 'bound'))],
     [('empty', T(lambda a: a['unproven'] == 0)), ('home_then_account', T(lambda a: a['unproven'] == 0))]),
 ('7.7 bytes accept rejects hold nothing', [('unread_rejected_bytes', T(lambda a: a['unread'] == 1))], [('pile', T(lambda a: a['unread'] == 0))]),
 ('7.5 the clamp compares tx', [('release_same_block_lesser_tx', T(lambda a: a['faces'][0]['spans'][0]['until'] == a['faces'][0]['spans'][0]['since']))], [('bind_then_release', T(lambda a: a['faces'][0]['spans'][0]['until'] != a['faces'][0]['spans'][0]['since']))]),
 ('7.5 state reads every span', [('later_of_two_bindings_released', T(lambda a: a['faces'][0]['state'] == 'bound'))], [('bind_then_release', T(lambda a: a['faces'][0]['state'] == 'released'))]),
 ('7.8 spans by since, tx before entry', [('spans_tx_before_entry', T(lambda a: a['faces'][0]['spans'][0]['since']['tx'][-1] == '1'))], []),
]

B += [
 ('6.1 chains compared numerically', [('valid_chains_nine_then_ten', tab)], [('invalid_chains_ten_then_nine', notab)]),
 ('7.1 steps compared numerically', [('steps_chain_nine_before_ten', T(lambda a: a['home'] is None and len(a['lineage']) == 2)), ('steps_block_nine_before_ten', T(lambda a: a['home'] == 1 and len(a['homes']) == 1)), ('steps_second_nine_before_ten', T(lambda a: a['home'] is None and len(a['lineage']) == 2))], []),
 ('7.3 moments compared numerically', [('moment_chain_nine_before_ten', T(lambda a: span0(a)['since']['chainId'] == 9)), ('moment_block_ninety_nine_before_hundred', T(lambda a: span0(a)['since']['blockNumber'] == 99))], []),
 ('7.2 an opening is a face entry of the home shape', [('home_then_account', T(lambda a: a['home'] == 1 and len(a['homes']) == 1))],
     [('home_body_of_another_type', T(lambda a: a['home'] is None and a['homes'] == [] and reasons(a) == ['UNSEATED'])), ('home_body_beyond_its_row', T(lambda a: a['home'] is None and a['homes'] == [] and reasons(a) == ['SHAPE', 'UNSEATED']))]),
 ('7.4 a release reads only under R', [('bind_then_release', T(lambda a: a['faces'][0]['state'] == 'released'))], [('release_under_another_root', T(lambda a: a['faces'][0]['state'] == 'bound' and reasons(a) == ['ROOT']))]),
 ('7.5 least moment before entry', [('release_least_moment_greater_entry', T(lambda a: span0(a)['until']['blockNumber'] == 20))], []),
 ('7.8 homes by record before entry', [('homes_by_record_before_entry', T(lambda a: len(a['homes']) == 2 and a['homes'][0]['blockNumber'] == 10 and a['homes'][0]['entry'] > a['homes'][1]['entry']))], []),
 ('7.8 homes one element per opening record', [('one_home_entry_opened_twice', T(lambda a: len(a['homes']) == 2 and a['homes'][0]['entry'] == a['homes'][1]['entry']))], []),
 ('7.7 unproven off the home and without bytes', [('unproven_off_home', T(lambda a: a['unproven'] == 1 and a['home'] == 1)), ('unproven_hash_unheld', T(lambda a: a['unproven'] == 1 and a['unread'] == 0))], []),
 ('7.6 ROOT before SIGNATURE and SUBJECT', [('reasons_root_before_signature_and_subject', T(lambda a: reasons(a) == ['ROOT', 'ROOT']))], []),
 ('2.2 key optional note', [('key', ent('key')), ('key_note', ent('key')), ('key_without_note_md', ent('key'))], [('key_note_int', noshape), ('key_note_null', noshape)]),
 ('2.2 release optional note', [('release', ent('release')), ('release_note', ent('release')), ('release_without_note_md', ent('release'))], [('release_note_int', noshape), ('release_note_null', noshape)]),
 ('2.2 home carries no note', [('home', ent('home'))], [('home_with_string_note', noshape), ('home_note_null', noshape)]),
 ('2.3 one face, one row', [('two_bindings_one_released', T(lambda a: len(a['faces']) == 1 and len(a['faces'][0]['spans']) == 2))],
     [('rows_in_order', T(lambda a: sum(1 for r in a['faces'] if r['kind'] == 'account' and r['platform'] == 'x' and r['handle'].lower() == 'alice') == 2))]),
 ('7.8 key rows by address before role', [('key_rows_address_before_role', T(lambda a: [r['role'] for r in a['faces']] == ['zz', 'aa'] and a['faces'][0]['address'] < a['faces'][1]['address']))], []),
]

B += [
 ('7.7 the counts read the office as the step found it',
     [('unproven_outgoing_office_in_handover_block', T(lambda a: a['unproven'] == 1)), ('unread_outgoing_office_in_handover_block', T(lambda a: a['unread'] == 1))],
     [('unproven_incoming_key_in_handover_block', T(lambda a: a['unproven'] == 0))]),
 ('7.7 unproven counts records, on every chain, of every type',
     [('unproven_one_hash_twice', T(lambda a: a['unproven'] == 2 and a['unread'] == 0)), ('unproven_entry_of_another_type', T(lambda a: a['unproven'] == 1)),
      ('unproven_move_of_the_home', T(lambda a: a['unproven'] == 1 and a['home'] == 1))], []),
 ('7.3 a release is seated on the home alone',
     [('release_on_new_home', T(lambda a: a['home'] == 10 and a['faces'][0]['state'] == 'released' and span0(a)['until']['chainId'] == 10))],
     [('release_off_home', T(lambda a: a['faces'][0]['state'] == 'bound' and reasons(a) == ['UNSEATED']))]),
 ('7.5 the least release ends the span wherever it falls',
     [('release_before_and_after_binding', T(lambda a: a['faces'][0]['state'] == 'released' and span0(a)['until'] == span0(a)['since']))], []),
 ('7.3 moment by block before tx in one second', [('moment_block_before_tx', T(lambda a: span0(a)['since']['blockNumber'] == 12))], []),
 ('6.1 record chainId and blockNumber are ints',
     [('home_book_alone', T(lambda a: len(a['lineage']) == 2))],
     [('invalid_anchor_chain_string', notab), ('invalid_accession_chain_string', notab), ('invalid_accession_block_string', notab)]),
]

B += [
 ('7.4 a seated entry that does not read is no binding',
     [('bind_then_release', T(lambda a: a['faces'][0]['state'] == 'released'))],
     [('release_of_seated_entries_that_do_not_read', T(lambda a: a['faces'] == [] and reasons(a) == ['ROOT', 'SIGNATURE', 'SUBJECT', 'SUBJECT']))]),
 ("7.2 an opening's author is the office as the step found it",
     [('home_by_office_after_handover', T(lambda a: a['home'] == 1 and len(a['homes']) == 1))],
     [('home_by_future_key', T(lambda a: a['home'] is None and a['homes'] == [] and len(a['lineage']) == 2 and reasons(a) == ['UNSEATED', 'UNSEATED'])),
      ('home_by_earlier_key', T(lambda a: a['home'] is None and a['homes'] == [] and len(a['lineage']) == 2 and reasons(a) == ['UNSEATED', 'UNSEATED'])),
      ('home_by_retired_key_moves_nothing', T(lambda a: a['home'] == 1 and len(a['homes']) == 1 and bound(a, 'still-home-ca') and reasons(a) == ['UNSEATED'])),
      ('home_by_retired_key_before_home', I(lambda a, v: tab(a) and a['home'] == 1 and len(a['homes']) == 1 and bound(a, 'after') and reasons(a) == ['UNSEATED']
                                              and a['homes'][0]['entry'] == [r['hash'] for r in v['anchors'] if r['blockNumber'] == 11][0]))]),
 ('declare key of sixty-four digits', [('signed', lambda a: dec(a) and 'sig' in a), ('signed_0x', lambda a: dec(a) and 'sig' in a)],
     [('declare_key_short', mis), ('declare_key_63_digits', mis), ('declare_key_65_digits', mis)]),
]
# every printable character as a one-character label, on its side of the label alphabet (sec 1)
LABEL = 'abcdefghijklmnopqrstuvwxyz0123456789-'
B += [('1 label alphabet, every printable character (%s)' % m,
       [('%s_char_%02x' % (m, c), ent(s)) for c in range(0x20, 0x7f) if chr(c) in LABEL],
       [('%s_char_%02x' % (m, c), noshape) for c in range(0x20, 0x7f) if chr(c) not in LABEL])
      for m, s in (('platform', 'account'), ('role', 'key'))]
B += [
 ('7.3 the author is the office as the step found it, in the hand-over block',
     [('author_outgoing_key_in_handover_block', T(lambda a: bound(a, 'outgoing-in-block') and len(a['lineage']) == 2 and a['misread'] == []))],
     [('author_incoming_key_in_handover_block', T(lambda a: a['faces'] == [] and reasons(a) == ['UNSEATED'])),
      ('opening_by_incoming_key_in_handover_block', T(lambda a: a['home'] is None and a['homes'] == [])),
      ('author_incoming_key_own_record_in_handover_block', T(lambda a: a['faces'] == [] and len(a['lineage']) == 2 and reasons(a) == ['UNSEATED']))]),
 ('9 a move back to an earlier home',
     [('home_moves_back', T(lambda a: a['home'] == 1 and len(a['homes']) == 3 and a['faces'][0]['state'] == 'bound')),
      ('home_moves_back_same_entry', T(lambda a: a['home'] == 1 and len(a['homes']) == 3 and a['faces'][0]['state'] == 'bound'))], []),
 ('7.5 the ending release by tx inside one block',
     [('two_releases_one_block_tx_decides', I(lambda a, v: tab(a) and span0(a)['release'] == [r['hash'] for r in v['anchors'] if r['tx'].endswith('1') and r['blockNumber'] == 30][0]))], []),
 ('7.8 homes by tx inside one block',
     [('two_openings_one_block_tx_decides', I(lambda a, v: tab(a) and a['homes'][0]['tx'].endswith('1') and a['homes'][0]['entry'] > a['homes'][1]['entry']))], []),
 ('7.3 an author in reach outside the line', [('author_in_reach_outside_line', T(lambda a: a['faces'] == [] and reasons(a) == ['AUTHOR']))], []),
 ('7.3 the office seats its own entry alone among the keys of the line',
     [('author_of_three_keys', T(lambda a: len(a['faces']) == 1 and bound(a, 'by-k2') and len(a['lineage']) == 3 and reasons(a) == ['UNSEATED', 'UNSEATED']))], []),
 ('7.3 a record seated at its step stays seated after its author hands over',
     [('seated_by_author_then_anchored_by_successor', T(lambda a: bound(a, 'twice') and len(a['faces'][0]['spans']) == 1 and span0(a)['since']['blockNumber'] == 12 and a['misread'] == [])),
      ('release_by_office_of_predecessors_binding', T(lambda a: a['faces'][0]['state'] == 'released' and span0(a)['until']['blockNumber'] == 21 and a['misread'] == []))], []),
 ('7.4 a release a retired key signed ends no span',
     [('author_retired_office_anchors_release', T(lambda a: bound(a, 'real') and span0(a)['release'] is None and reasons(a) == ['UNSEATED']))], []),
 ('9 the first home entry anchored again by a successor moves nothing back',
     [('home_entry_anchored_again_by_successor', T(lambda a: a['home'] == 10 and len(a['homes']) == 2 and len(a['lineage']) == 2 and bound(a, 'stays-on-chain-10')))], []),
]

# the head of a chain's book, the opening block, the persona
def pers(a): return a['persona']['name_md'] if a['persona'] else None
def prow(a, name):
    rows = [r for r in a['faces'] if r['kind'] == 'persona' and r['name_md'] == name]
    return rows[0] if len(rows) == 1 else {'state': None, 'spans': []}
BARRED = [(0x0000, 0x0020), (0x007F, 0x00A0), (0x00AD, 0x00AD), (0x034F, 0x034F), (0x0600, 0x0605),
          (0x061C, 0x061C), (0x06DD, 0x06DD), (0x070F, 0x070F), (0x0890, 0x0891), (0x08E2, 0x08E2),
          (0x115F, 0x1160), (0x1680, 0x1680), (0x17B4, 0x17B5), (0x180B, 0x180F), (0x2000, 0x200F),
          (0x2028, 0x202F), (0x205F, 0x206F), (0x2800, 0x2800), (0x3000, 0x3000), (0x3164, 0x3164),
          (0xE000, 0xF8FF), (0xFDD0, 0xFDEF), (0xFE00, 0xFE0F), (0xFEFF, 0xFEFF), (0xFFA0, 0xFFA0),
          (0xFFF0, 0xFFFF), (0x110BD, 0x110BD), (0x110CD, 0x110CD), (0x13430, 0x1343F),
          (0x13441, 0x13442), (0x16FE4, 0x16FE4), (0x1BC9D, 0x1BC9D), (0x1BCA0, 0x1BCA3), (0x1D159, 0x1D159), (0x1D173, 0x1D17A), (0xE0000, 0xE0FFF), (0xF0000, 0x10FFFF)]
def _barred(o): return o % 0x10000 >= 0xFFFE or any(lo <= o <= hi for lo, hi in BARRED)
def _free(o, step):
    o += step
    while 0 <= o <= 0x10FFFF and (0xD800 <= o <= 0xDFFF or _barred(o)):
        o += step
    return o if 0 <= o <= 0x10FFFF else None
PLANE_END = ([0x1FFFE, 0x1FFFF, 0xEFFFE], [0x1FFFD, 0x20000])
B += [
 ("7.2 the head of a chain's book follows the records under R alone, whoever their holder",
     [('head_follows_any_holder_under_root', T(lambda a: a['home'] == 10 and len(a['lineage']) == 2 and bound(a, 'head-by-any-holder')))],
     [('head_ignores_another_root', T(lambda a: a['home'] is None and a['homes'] == [] and absent(a, 'head-under-another-root')))]),
 ("7.2 the opening block's accessions are read under the new home",
     [('opening_block_accession_under_new_home', T(lambda a: a['home'] == 10 and len(a['lineage']) == 3 and bound(a, 'k2-after-the-opening-block') and absent(a, 'k1-after-the-opening-block'))),
      ('opening_and_accession_moving_block', T(lambda a: a['home'] == 10 and len(a['lineage']) == 2 and bound(a, 'k1-after-the-block') and absent(a, 'root-after-the-block')))],
     [('old_home_accession_in_the_second_of_the_move', T(lambda a: a['home'] == 1 and len(a['lineage']) == 1 and bound(a, 'root-on-the-new-home')))]),
 ("9 a move back needs the old home's book in step",
     [('move_back_needs_the_book_in_step', T(lambda a: a['home'] == 1 and len(a['homes']) == 3 and len(a['lineage']) == 2 and bound(a, 'back-once-the-book-is-in-step') and reasons(a) == ['UNSEATED']))], []),
 ('2.2 persona exact members', [('persona', ent('persona')), ('persona_cjk', ent('persona'))],
     [(s, noshape) for s in ('persona_with_note', 'persona_extra_member', 'persona_without_act', 'persona_without_kind', 'persona_without_name_md',
                             'persona_without_root', 'persona_with_handle', 'persona_name_plain_member', 'persona_act_release', 'persona_kind_cased')]),
 ('1 name is a string', [('persona', ent('persona'))],
     [(s, noshape) for s in ('persona_name_int', 'persona_name_null', 'persona_name_array', 'persona_name_object', 'persona_name_bool')]),
 ('1 name length 1..32', [(s, ent('persona')) for s in ('persona_one', 'persona_len32', 'persona_len32_cjk', 'persona_len32_astral')],
     [(s, noshape) for s in ('persona_empty', 'persona_len33', 'persona_len33_cjk', 'persona_len33_astral')]),
 ('1 name without a space or a line break', [('persona_punctuation', ent('persona')), ('persona_combining', ent('persona'))],
     [(s, noshape) for s in ('persona_space_inside', 'persona_space_first', 'persona_space_last', 'persona_two_lines')]),
 ('1 name outside ASCII rides a prose member', [('persona_cjk', ent('persona'))], [('persona_name_plain_member_cjk', rej)]),
]
B += [('1 name barred U+%04X to U+%04X' % (lo, hi),
       [('persona_free_%04x' % o, ent('persona')) for o in (_free(lo, -1), _free(hi, 1)) if o is not None],
       [('persona_barred_%04x' % o, noshape) for o in ([lo] if lo == hi else [lo, hi])])
      for lo, hi in BARRED]
B += [
 ('1 name barred: the last two scalar values of a plane',
     [('persona_free_%04x' % o, ent('persona')) for o in PLANE_END[1]], [('persona_barred_%04x' % o, noshape) for o in PLANE_END[0]]),
 ('7.4 a persona entry stands like a binding',
     [('persona_alone', T(lambda a: pers(a) == '圣山契约行会' and [r['kind'] for r in a['faces']] == ['persona'] and a['faces'][0]['state'] == 'bound' and a['persona']['entry'] == span0(a)['entry']))],
     [('persona_by_retired_key', T(lambda a: a['persona'] is None and a['faces'] == [] and reasons(a) == ['UNSEATED'])),
      ('persona_off_the_home', T(lambda a: a['persona'] is None and a['faces'] == [] and reasons(a) == ['UNSEATED'])),
      ('persona_before_a_home', T(lambda a: a['persona'] is None and a['faces'] == [] and reasons(a) == ['UNSEATED'])),
      ('persona_under_another_root', T(lambda a: a['persona'] is None and a['faces'] == [] and reasons(a) == ['ROOT'])),
      ('persona_void_record', T(lambda a: a['persona'] is None and a['faces'] == [] and reasons(a) == ['UNSEATED'])),
      ('persona_name_with_a_space', T(lambda a: a['persona'] is None and a['faces'] == [] and reasons(a) == ['SHAPE']))]),
 ('7.5 the persona: the open span last in since',
     [('persona_latest_of_two', T(lambda a: pers(a) == 'second' and prow(a, 'first')['state'] == 'bound' and prow(a, 'second')['state'] == 'bound')),
      ('persona_falls_back_on_release', T(lambda a: pers(a) == 'first' and prow(a, 'second')['state'] == 'released')),
      ('persona_release_of_the_earlier', T(lambda a: pers(a) == 'second' and prow(a, 'first')['state'] == 'released')),
      ('persona_bound_again', T(lambda a: pers(a) == 'alpha' and len(prow(a, 'alpha')['spans']) == 2 and a['persona']['entry'] == prow(a, 'alpha')['spans'][1]['entry'] and prow(a, 'alpha')['spans'][0]['release'] is not None)),
      ('persona_row_of_two_open_spans', T(lambda a: pers(a) == 'alpha' and a['persona']['entry'] == prow(a, 'alpha')['spans'][1]['entry'] and prow(a, 'beta')['state'] == 'bound')),
      ('persona_row_later_span_closed', T(lambda a: pers(a) == 'beta' and prow(a, 'alpha')['state'] == 'bound')),
      ('persona_across_a_handover', T(lambda a: pers(a) == 'old-name' and len(a['lineage']) == 2 and prow(a, 'new-name')['state'] == 'released'))], []),
 ('7.5 the persona: none where no span of a persona row is open',
     [('persona_all_released', T(lambda a: a['persona'] is None and prow(a, 'first')['state'] == 'released' and prow(a, 'second')['state'] == 'released')),
      ('account_on_a_platform_called_persona', T(lambda a: a['persona'] is None and [r['kind'] for r in a['faces']] == ['account']))], []),
 ('7.5 the persona: chains at one second by chain id',
     [('persona_one_second_two_chains', T(lambda a: pers(a) == 'on-chain-10' and a['home'] == 10 and prow(a, 'on-chain-1')['state'] == 'bound'
                                            and prow(a, 'on-chain-1')['spans'][0]['since']['blockTimestamp'] == prow(a, 'on-chain-10')['spans'][0]['since']['blockTimestamp']))], []),
 ('7.5 the persona: equal moments by the greater entry',
     [('persona_one_transaction_two_entries', I(lambda a, v: tab(a) and len(a['faces']) == 2 and a['persona']['entry'] == max(r['hash'] for r in v['anchors'] if r['blockNumber'] == 12)))], []),
 ('7.8 rows: accounts, keys, personas, the personas bytewise by name',
     [('persona_rows_in_order', T(lambda a: [r['kind'] for r in a['faces']] == ['account', 'key'] + ['persona'] * 5
                                    and [r['name_md'] for r in a['faces'][2:]] == ['a', 'b', 'é', '中', '\U0001F600']))], []),
 ('7.8 a name spelled as the canonical form spells it',
     [('persona_spelled_in_the_table', T(lambda a: pers(a) == 'q"\\'))], []),
]

# the corpus reader's cases of the second fold
PLANE_END_ALL = ([p * 0x10000 + d for p in range(2, 15) for d in (0xFFFE, 0xFFFF) if p * 0x10000 + d != 0xEFFFE],
                 [p * 0x10000 + 0xFFFD for p in range(2, 13)])
def names(a): return [r['name_md'] for r in a['faces'] if r['kind'] == 'persona']
B += [
 ('1 name barred: the last two scalar values of every plane from 2 to 14',
     [('persona_free_%04x' % o, ent('persona')) for o in PLANE_END_ALL[1]], [('persona_barred_%04x' % o, noshape) for o in PLANE_END_ALL[0]]),
 ('1 name length in scalar values, none grouped or normalized', [('persona_len32_combining', ent('persona')), ('persona_len32_mixed_widths', ent('persona'))],
     [('persona_len33_combining', noshape), ('persona_len33_mixed_widths', noshape)]),
 ('7.8 persona rows bytewise by UTF-8, never by UTF-16 code units',
     [('persona_rows_utf8_not_utf16', T(lambda a: names(a) == ['\uf900', '\uff01', '\U00010000', '\U0001f600'] and pers(a) == '\U00010000'))], []),
 ('2.3 names compare bytewise, none normalized or folded',
     [('persona_rows_not_normalized', T(lambda a: names(a) == ['Alice', 'alice', 'e\u0301', '\u00e9'] and pers(a) == 'alice'))], []),
 ('7.5 the persona: tx before entry inside one block',
     [('persona_one_block_tx_decides', I(lambda a, v: tab(a) and a['persona'] is not None and a['persona']['entry'] == [r['hash'] for r in v['anchors'] if r['tx'].endswith('2') and r['blockNumber'] == 12][0]
                                          and a['persona']['entry'] < [r['hash'] for r in v['anchors'] if r['tx'].endswith('1') and r['blockNumber'] == 12][0]))], []),
 ('7.5 the persona: block before tx inside one second',
     [('persona_one_second_block_decides', I(lambda a, v: tab(a) and a['persona'] is not None and a['persona']['entry'] == [r['hash'] for r in v['anchors'] if r['blockNumber'] == 31][0]
                                              and a['persona']['entry'] < [r['hash'] for r in v['anchors'] if r['blockNumber'] == 30][0]))], []),
 ('6.1 anchor blockNumber an int', [('home_then_account', tab)], [('invalid_anchor_block_string', notab), ('invalid_anchor_block_string_alone', notab)]),
 ('7.5 the persona: chain before block inside one second',
     [('persona_one_second_chain_before_block', T(lambda a: pers(a) == 'on-chain-10-block-40' and a['home'] == 10 and prow(a, 'on-chain-1-block-50')['state'] == 'bound'))], []),
]
BARRED_INSIDE = [0x2065, 0xFFFC, 0xFFFD, 0xE0100]
B += [
 ('1 name barred inside a range, away from its ends', [('persona_barred_%04x' % o, noshape) for o in BARRED_INSIDE], []),
 ('1 name length counts each regional indicator', [('persona_len32_regional_indicators', ent('persona'))], [('persona_len33_regional_indicators', noshape)]),
 ('7.2 a record under another root leaves the head as it stood',
     [('head_kept_through_another_roots_record', I(lambda a, v: tab(a) and a['home'] == 10 and len(a['lineage']) == 2 and bound(a, 'head-kept')
                                                    and any(x['root'] != v['root'] and x['chainId'] == 10 for x in v['accessions'])))], []),
 ('7.2 an opening on the standing home is tested by the head',
     [('opening_again_on_the_home_needs_the_head', I(lambda a, v: tab(a) and a['home'] == 1 and len(a['homes']) == 1 and len(a['lineage']) == 1 and bound(a, 'still-home')
                                                      and reasons(a) == ['UNSEATED'] and len(_home_hashes(v)) == 2))], []),
 ('7.8 persona rows bytewise, no case folded', [('persona_rows_bytewise_not_caseless', T(lambda a: names(a) == ['Zed', 'alpha'] and pers(a) == 'Zed'))], []),
]
def _name_is(nm, test):
    return I(lambda a, v: bool(test(a)) and isinstance(v, dict) and isinstance(v.get('body'), dict) and v['body'].get('name_md') == nm)
B += [
 ('1 name length counts decoded scalar values, an escape counting one',
     [('persona_len32_quote', _name_is('a' * 31 + '"', ent('persona'))), ('persona_len32_backslashes', _name_is('\\' * 32, ent('persona')))],
     [('persona_len33_quote', _name_is('a' * 32 + '"', noshape)), ('persona_len33_backslashes', _name_is('\\' * 33, noshape))]),
 ('1 a name may begin with a combining mark', [('persona_leading_combining', _name_is('\u0301a', ent('persona')))], []),
]
# a persona edge case passes its side only where its input is the name the case is named for
def _edge(slug, test):
    m_ = re.match(r'^persona_(barred|free)_([0-9a-f]+)$', slug)
    if not m_:
        return test
    nm = 'a' + chr(int(m_.group(2), 16)) + 'b'
    return I(lambda a, v: bool(test(a)) and isinstance(v, dict) and isinstance(v.get('body'), dict) and v['body'].get('name_md') == nm)
B = [(n_, [(s, _edge(s, t)) for s, t in sa], [(s, _edge(s, t)) for s, t in sb]) for n_, sa, sb in B]
# the cases of the opening's rules pass their side only where their input holds the step the rule reads
def _home_hashes(v):
    out = set()
    for x in v['pile']:
        try:
            b_ = bytes.fromhex(x[2:])
            j_ = json.loads(b_)
            if isinstance(j_, dict) and isinstance(j_.get('body'), dict) and j_['body'].get('act') == 'home':
                out.add('0x' + hashlib.sha256(b_).hexdigest())
        except Exception:
            pass
    return out
def _same_block_entry(a, v):
    h_ = _home_hashes(v)
    return any(r['hash'] in h_ and x['chainId'] == r['chainId'] and x['blockNumber'] == r['blockNumber'] and x['to'] == r['sender'] and x['root'] == v['root']
               for r in v['anchors'] for x in v['accessions'])
def _opening_block_handover(a, v):
    return bool(a['homes']) and any(x['chainId'] == a['homes'][-1]['chainId'] and x['blockNumber'] == a['homes'][-1]['blockNumber']
                                    and x['holder'] == a['homes'][-1]['sender'] and x['root'] == v['root'] for x in v['accessions'])
NEEDS = {'opening_in_the_block_of_entering': _same_block_entry,
         'opening_block_accession_under_new_home': _opening_block_handover,
         'opening_and_accession_moving_block': _opening_block_handover}
def _needs(slug, test):
    f = NEEDS.get(slug)
    if f is None:
        return test
    return I(lambda a, v: bool(test.f(a, v) if isinstance(test, I) else test(a)) and isinstance(v, dict) and f(a, v))
B = [(n_, [(s, _needs(s, t)) for s, t in sa], [(s, _needs(s, t)) for s, t in sb]) for n_, sa, sb in B]

# ---- the reroot and origin shapes, the seal, the two members that name them, and joined
KEY = lambda tag: m['keys'][tag]['address']
TXLO, TXMID, TXHI = '0x' + '11' * 32, '0x' + '88' * 32, '0x' + 'ee' * 32
rr = lambda a: a['reroot']['to'] if a['reroot'] else None
og = lambda a: a['origin']['from'] if a['origin'] else None
J = lambda v_: (lambda a: bool(a) and a.get('joined') is v_)
def _ids(v, act):
    out = set()
    for x in v['pile']:
        try:
            b_ = bytes.fromhex(x[2:])
            j_ = json.loads(b_)
            if isinstance(j_, dict) and isinstance(j_.get('body'), dict) and j_['body'].get('act') == act:
                out.add('0x' + hashlib.sha256(b_).hexdigest())
        except Exception:
            pass
    return out
def _row(s_, o_):
    plain = lambda a: ent(s_)(a) and a['rooted'] is True and a['consent'] is None
    f = lambda x: x % dict(s=s_, o=o_)
    return [
     (f('2.2 %(s)s row: exactly act, root and %(o)s, with or without note_md'),
      [(f('%(s)s_plain'), plain), (f('%(s)s_with_note'), plain), (f('%(s)s_note_empty'), plain)],
      [(f(x), noshape) for x in ('%(s)s_extra_member', '%(s)s_with_sig', '%(s)s_with_kind', '%(s)s_missing_%(o)s', '%(s)s_both_to_and_from', '%(s)s_root_absent')]),
     (f('2.2 %(s)s row: %(o)s a hex20'), [(f('%(s)s_plain'), plain)],
      [(f(x), noshape) for x in ('%(s)s_%(o)s_short', '%(s)s_%(o)s_long', '%(s)s_%(o)s_upper', '%(s)s_%(o)s_bare', '%(s)s_%(o)s_int', '%(s)s_%(o)s_null')]),
     (f('2.2 %(s)s row: root a hex20'), [(f('%(s)s_plain'), plain)], [(f('%(s)s_root_short'), noshape)]),
     (f('2.2 %(s)s row: %(o)s byte-unequal to root'), [(f('%(s)s_differs_in_one_digit'), plain), (f('%(s)s_plain'), plain)], [(f('%(s)s_%(o)s_is_root'), noshape)]),
     (f('2.2 %(s)s row: note_md a string'), [(f('%(s)s_with_note'), plain)], [(f('%(s)s_note_int'), noshape), (f('%(s)s_note_null'), noshape)]),
     (f('2.2 %(s)s row: act the string %(s)s'), [(f('%(s)s_plain'), plain)], [(f('%(s)s_act_capital'), noshape), (f('%(s)s_act_with_bind_kind'), noshape)]),
     (f('2.2 %(s)s row: the shape reads no author, and rooted says whose it is'),
      [(f('%(s)s_by_another_key'), plain), (f('%(s)s_other_root'), lambda a: ent(s_)(a) and a['rooted'] is False)], [])]
B += _row('reroot', 'to') + _row('origin', 'from')
_open = lambda handle, why: T(lambda a: a['reroot'] is None and bound(a, handle) and reasons(a) == why)
_shut = lambda pred: T(lambda a: a['reroot'] is not None and pred(a))
_noorigin = lambda why: T(lambda a: a['origin'] is None and reasons(a) == why)
_seals = T(lambda a: rr(a) == KEY('root2') and bound(a, 'before-the-seal') and absent(a, 'after-the-seal') and reasons(a) == ['UNSEATED'])
def _lesser_tx_greater_entry(member, act):
    return I(lambda a, v: tab(a) and len(_ids(v, act)) == 2 and a[member]['entry'] == max(_ids(v, act)) and a[member]['since']['tx'] == TXLO
             and sorted(r['tx'] for r in v['anchors'] if r['hash'] in _ids(v, act)) == sorted([TXLO, [r['tx'] for r in v['anchors'] if r['hash'] == min(_ids(v, act))][0]])
             and reasons(a) == [])
B += [
 ('2.2 reroot row: a from in the place of to', [('reroot_plain', ent('reroot'))], [('reroot_with_from', noshape)]),
 ('2.2 origin row: a to in the place of from', [('origin_plain', ent('origin'))], [('origin_with_to', noshape)]),
 ('7.2 the seal closes after a seated reroot that reads',
  [('reroot_seals', _seals)],
  [('no_reroot_no_seal', T(lambda a: a['reroot'] is None and bound(a, 'no-reroot-first') and bound(a, 'no-reroot-second') and reasons(a) == []))]),
 ("7.2 the seal is read as the step found it: the reroot's own block",
  [('reroot_block_binds_on_both_sides', _shut(lambda a: bound(a, 'same-block-lesser-tx') and bound(a, 'same-block-greater-tx')
                                                and absent(a, 'after-the-block-of-bindings') and reasons(a) == ['UNSEATED'])),
   ('reroot_block_counts_unread_and_unproven', _shut(lambda a: a['unread'] == 1 and a['unproven'] == 1)),
   ('origin_in_the_block_of_the_reroot', T(lambda a: rr(a) == KEY('root2') and og(a) == KEY('k3') and absent(a, 'after-the-block-of-the-origin')
                                             and reasons(a) == ['UNSEATED']))], []),
 ('7.2 the step after the reroot: time, then chainId',
  [('sealed_before_an_opening_of_the_same_second', _shut(lambda a: a['home'] == 1 and len(a['homes']) == 1 and reasons(a) == ['UNSEATED']))],
  [('opening_of_the_same_second_unseats_the_reroot', T(lambda a: a['reroot'] is None and a['home'] == 1 and len(a['homes']) == 2
                                                         and bound(a, 'bound-on-the-new-home') and reasons(a) == ['UNSEATED']))]),
 ('7.2 a reroot that does not read seals nothing', [('reroot_seals', _seals)],
  [('reroot_under_another_root_seals_nothing', _open('after-anothers-reroot', ['ROOT'])),
   ('reroot_to_the_root_itself_seals_nothing', _open('after-a-reroot-to-itself', ['SHAPE']))]),
 ('7.2 a reroot seals by a seated record: on the home',
  [('reroot_seals', T(lambda a: _seals(a) and a['reroot']['since']['chainId'] == a['home'])),
   ('reroot_on_a_home_moved_to', _shut(lambda a: a['home'] == 10 and a['reroot']['since']['chainId'] == 10 and len(a['homes']) == 2
                                        and a['office'] == KEY('k1') and reasons(a) == ['UNSEATED']))],
  [('reroot_off_the_home', _open('after-a-reroot-off-the-home', ['UNSEATED'])),
   ('reroot_before_a_home', _open('after-a-reroot-before-the-home', ['UNSEATED'])),
   ('reroot_in_the_block_that_opens_its_chain', T(lambda a: a['reroot'] is None and a['home'] == 10 and bound(a, 'bound-on-the-chain-moved-to')
                                                    and reasons(a) == ['UNSEATED']))]),
 ('7.2 a reroot seals by a seated record: its author and its sender the office',
  [('reroot_by_a_later_office', _shut(lambda a: a['office'] == KEY('k1') and absent(a, 'after-the-offices-reroot') and reasons(a) == ['UNSEATED'])),
   ('reroot_in_the_block_of_the_handover', _shut(lambda a: a['office'] == KEY('k1') and len(a['lineage']) == 2
                                                  and absent(a, 'after-the-outgoing-keys-reroot') and reasons(a) == ['UNSEATED']))],
  [('reroot_signed_by_the_key_handed_on_from', _open('after-a-predecessors-reroot', ['UNSEATED'])),
   ('reroot_sent_by_a_retired_key', _open('after-a-retired-senders-reroot', ['UNSEATED'])),
   ('reroot_by_a_retired_key', _open('after-a-retired-keys-reroot', ['UNSEATED'])),
   ('reroot_by_an_outsider', _open('after-an-outsiders-reroot', [])),
   ('reroot_by_the_incoming_key_in_its_accession_block', _open('after-the-incoming-keys-reroot', ['UNSEATED']))]),
 ('7.2 a reroot seals by a seated record: counted', [('reroot_seals', _seals)],
  [('reroot_void', _open('after-a-reroot-void', ['UNSEATED'])),
   ('reroot_unproven', T(lambda a: a['reroot'] is None and a['unproven'] == 1 and bound(a, 'after-a-reroot-unproven') and reasons(a) == ['UNSEATED']))]),
 ('7.2 a reroot seals by an entry of the pile', [('reroot_seals', _seals)],
  [('reroot_without_its_bytes', T(lambda a: a['reroot'] is None and a['unread'] == 1 and bound(a, 'after-a-reroot-without-bytes') and reasons(a) == []))]),
 ('7.2 a sealed seat holds no record in office: unread', [('sealed_unread_stops', _shut(lambda a: a['unread'] == 1))], []),
 ('7.2 a sealed seat holds no record in office: unproven', [('sealed_unproven_stops', _shut(lambda a: a['unproven'] == 1))], []),
 ('7.2 a sealed seat holds no record in office: no release, no persona',
  [('sealed_release_ends_nothing', _shut(lambda a: bound(a, 'kept-bound-by-the-seal') and reasons(a) == ['UNSEATED'])),
   ('sealed_persona_stays', _shut(lambda a: pers(a) == 'before' and reasons(a) == ['UNSEATED']))], []),
 ('7.2 a sealed seat holds no record in office: no opening',
  [('sealed_home_stays', _shut(lambda a: a['home'] == 1 and len(a['homes']) == 1 and reasons(a) == ['UNSEATED'])),
   ('sealed_home_opens_no_more', _shut(lambda a: a['home'] == 1 and len(a['homes']) == 1 and reasons(a) == ['UNSEATED']))], []),
 ('7.2 the accessions of the home move the office and the line of a sealed seat',
  [('sealed_office_follows_the_book', _shut(lambda a: a['office'] == KEY('k1') and len(a['lineage']) == 2
                                             and absent(a, 'bound-by-a-key-seated-after-the-seal') and reasons(a) == ['UNSEATED'])),
   ('sealed_line_grows', _shut(lambda a: a['office'] == KEY('k2') and len(a['lineage']) == 3 and reasons(a) == ['UNSEATED']))],
  [('sealed_office_stays_for_another_chains_book', _shut(lambda a: a['office'] == a['root'] and len(a['lineage']) == 1))]),
 ('7.2 a reroot reads no head', [('reroot_reads_no_head', T(lambda a: rr(a) == KEY('root2') and len(a['lineage']) == 1))], []),
 ('7.4 the reroot: tx inside one block',
  [('two_reroots_one_block_tx_decides', _lesser_tx_greater_entry('reroot', 'reroot'))], []),
 ('7.4 the reroot: entry_id inside one tx',
  [('two_reroots_one_tx_entry_decides', I(lambda a, v: tab(a) and len(_ids(v, 'reroot')) == 2 and a['reroot']['entry'] == min(_ids(v, 'reroot'))
                                           and len(set(r['tx'] for r in v['anchors'] if r['hash'] in _ids(v, 'reroot'))) == 1 and reasons(a) == []))], []),
 ("7.4 the reroot's since: its least seated record",
  [('reroot_anchored_twice_since_least_tx', T(lambda a: a['reroot']['since']['tx'] == TXLO)),
   ('reroot_since_is_its_seated_record', T(lambda a: a['reroot']['since']['chainId'] == 1 and a['reroot']['since']['blockNumber'] == 20))], []),
 ('7.4 a later reroot: seated where the seal is open',
  [('second_reroot_after_a_void_first', T(lambda a: rr(a) == KEY('k3') and reasons(a) == ['UNSEATED']))],
  [('second_reroot_after_the_seal', T(lambda a: rr(a) == KEY('root2') and reasons(a) == ['UNSEATED']))]),
 ('7.4 the reroot is chosen among the reroots that read',
  [('two_reroots_the_lesser_tx_under_another_root', T(lambda a: rr(a) == KEY('root2') and a['reroot']['since']['tx'] == TXHI and reasons(a) == ['ROOT']))], []),
 ('7.4 an origin stands by a seated record, and seals nothing',
  [('origin_stands_and_seals_nothing', T(lambda a: og(a) == KEY('root2') and a['reroot'] is None and bound(a, 'after-an-origin') and reasons(a) == []))],
  [('origin_off_the_home', _noorigin(['UNSEATED'])), ('origin_before_a_home', _noorigin(['UNSEATED'])),
   ('origin_signed_by_the_key_handed_on_from', _noorigin(['UNSEATED'])),
   ('origin_after_the_seal', T(lambda a: a['origin'] is None and a['reroot'] is not None and reasons(a) == ['UNSEATED']))]),
 ('7.4 an origin reads under R', [('origin_stands_and_seals_nothing', T(lambda a: og(a) == KEY('root2')))],
  [('origin_under_another_root', _noorigin(['ROOT'])), ('origin_from_the_root_itself', _noorigin(['SHAPE']))]),
 ('7.4 the origin: the least moment, then the lesser entry_id',
  [('two_origins_the_earlier', T(lambda a: og(a) == KEY('root2') and a['origin']['since']['blockNumber'] == 12 and reasons(a) == [])),
   ('two_origins_one_block_tx_decides', _lesser_tx_greater_entry('origin', 'origin')),
   ('two_origins_one_tx_entry_decides', I(lambda a, v: tab(a) and len(_ids(v, 'origin')) == 2 and a['origin']['entry'] == min(_ids(v, 'origin')))),
   ('origin_after_a_void_first', T(lambda a: og(a) == KEY('k3') and reasons(a) == ['UNSEATED']))], []),
 ("7.4 the origin's since: its least seated record",
  [('origin_since_is_its_least_seated_record', T(lambda a: a['origin']['since']['chainId'] == 1 and a['origin']['since']['blockNumber'] == 14))], []),
 ('7.8 a reroot and an origin in one table',
  [('origin_and_reroot_in_one_table', T(lambda a: og(a) == KEY('k3') and rr(a) == KEY('root2'))),
   ('origin_and_reroot_with_notes', T(lambda a: og(a) == KEY('root2') and rr(a) == KEY('k3')))], []),
 ('7.4 a release names bindings alone: a reroot or an origin is none',
  [('release_names_a_reroot', T(lambda a: rr(a) == KEY('root2') and reasons(a) == ['SUBJECT'])),
   ('release_in_the_block_of_the_reroot_it_names', T(lambda a: rr(a) == KEY('root2') and reasons(a) == ['SUBJECT'])),
   ('release_names_an_origin', T(lambda a: og(a) == KEY('root2') and reasons(a) == ['SUBJECT']))], []),
 ("7.9 joined: the first's reroot names the second's root", [('joined_both_name_each_other', J(True))],
  [(x, J(False)) for x in ('joined_needs_the_reroot', 'joined_reroot_names_another', 'joined_reads_the_first_reroot', 'joined_needs_a_seated_reroot')]),
 ("7.9 joined: the second's origin names the first's root",
  [('joined_both_name_each_other', J(True)),
   ('joined_by_a_later_office_of_the_second', I(lambda a, v: J(True)(a) and any(x['root'] == v[1]['root'] and x['holder'] == v[1]['root'] for x in v[1]['accessions'])))],
  [(x, J(False)) for x in ('joined_needs_the_origin', 'joined_origin_names_another', 'joined_reads_the_earlier_origin', 'joined_needs_an_origin_by_the_office')]),
 ('7.9 joined is ordered, the first to the second', [('joined_both_name_each_other', J(True))],
  [(x, J(False)) for x in ('joined_not_in_the_other_order', 'joined_one_input_twice', 'joined_neither')]),
 ('7.9 joined link by link', [('joined_first_link_of_three', J(True)), ('joined_second_link_of_three', J(True))], [('joined_not_across_the_middle', J(False))]),
 ('7.9 joined reads each table under its own input',
  [('joined_over_two_bases', I(lambda a, v: J(True)(a) and not set(c['chainId'] for c in v[0]['chains']) & set(c['chainId'] for c in v[1]['chains']))),
   ('joined_whatever_the_first_office_does_after', I(lambda a, v: J(True)(a) and any(x['root'] == v[0]['root'] and x['holder'] == v[0]['root'] for x in v[0]['accessions'])))], []),
 ('7.4 an origin stands by a seated record: its author and its sender the office, counted, its bytes in the pile',
  [('origin_by_a_later_office', T(lambda a: og(a) == KEY('root2') and a['office'] == KEY('k1') and reasons(a) == []))],
  [('origin_sent_by_a_retired_key', _noorigin(['UNSEATED'])), ('origin_by_a_retired_key', _noorigin(['UNSEATED'])),
   ('origin_by_an_outsider', _noorigin([])), ('origin_void', _noorigin(['UNSEATED'])),
   ('origin_unproven', T(lambda a: a['origin'] is None and a['unproven'] == 1 and reasons(a) == ['UNSEATED'])),
   ('origin_without_its_bytes', T(lambda a: a['origin'] is None and a['unread'] == 1 and reasons(a) == [])),
   ('origin_by_the_incoming_key_in_its_accession_block', T(lambda a: a['origin'] is None and a['office'] == KEY('k1') and reasons(a) == ['UNSEATED']))]),
 ("7.4 the origin's since: tx inside one block",
  [('origin_anchored_twice_since_least_tx', I(lambda a, v: tab(a) and a['origin']['since']['tx'] == TXLO and [r['tx'] for r in v['anchors'] if r['hash'] in _ids(v, 'origin')] == [TXHI, TXLO]))], []),
 ('7.2 the heads of a sealed seat', [('sealed_office_stays_for_another_chains_book', _shut(lambda a: a['office'] == a['root'] and len(a['lineage']) == 1))], []),
 ("7.2 a key a reader's clocks hold in office reroots",
  [('reroot_by_a_retired_key_on_a_chain_that_runs_behind', T(lambda a: rr(a) == KEY('thief') and a['home'] == 10 and a['office'] == a['root']
                                                              and len(a['lineage']) == 1 and reasons(a) == []))], []),
 ('7.9 joined is read under two bases',
  [('joined_under_a_basis_that_leaves_out_the_home', I(lambda a, v: J(True)(a) and len(v[0]['chains']) == 1)),
   ('joined_under_the_basis_that_holds_the_home', I(lambda a, v: J(True)(a) and len(v[0]['chains']) == 3))],
  [('joined_not_under_the_basis_that_holds_the_home', I(lambda a, v: J(False)(a) and len(v[0]['chains']) == 3))]),
 ("harness joined: the depth bound counts from the pair's array",
  [('joined_pair_depth_128', I(lambda a, v: J(True)(a) and _depth(v) == 128))],
  [('joined_pair_depth_129', I(lambda a, v: notab(a) and _depth(v) == 129 and _depth(v[0]) == 128))]),
 ('harness joined: an array of exactly two valid face inputs', [('joined_both_name_each_other', J(True)), ('joined_whitespace_around', J(True))],
  [(x, notab) for x in ('joined_first_no_input', 'joined_second_no_input', 'joined_one_element', 'joined_three_elements', 'joined_empty_array',
                        'joined_a_single_input', 'joined_null', 'joined_second_null', 'joined_first_an_array', 'joined_an_object_of_two',
                        'joined_byte_order_mark', 'joined_trailing_bytes', 'joined_empty_file')]),
]
BOUNDARY_DATA = [[name, [x for x, _ in a], [x for x, _ in b]] for name, a, b in B]
# the boundaries whose two sides give one answer on every input (law sec 10.1), with the reason
UNOBSERVABLE = [
 [
  "3.3 s <= (n-1)/2",
  "a consent with s at (n - 1) / 2 that holds needs an address fixed by the hash of a declaration that names that address, so both sides answer false"
 ],
 [
  "5.1 length > 36",
  "a tail of 36 bytes or fewer is no entry under accept, so a calldata of 36 bytes and one of 37 bytes whose tail is no entry both answer null"
 ],
 [
  "7.2 the heads of a sealed seat",
  "no record is in office after the seal, so no opening and no seating of a home entry reads a head, and a walk that set the heads of a sealed seat and one that left them give one table"
 ]
]


def labels_of(cmd, answer):
    if answer is None:
        return ['misuse']
    out = []
    if cmd == 'entry':
        if not answer['ok']:
            return ['entry:rejected']
        if not answer['face']:
            return ['entry:other-type']
        out.append('shape:' + (answer['shape'] or 'none'))
        if answer['rooted'] is not None:
            out.append('rooted:' + ('true' if answer['rooted'] else 'false'))
        if answer['consent'] is not None:
            out.append('consent:' + ('true' if answer['consent'] else 'false'))
    elif cmd == 'declare':
        out.append('declare:signed' if 'sig' in answer else 'declare:plain')
    elif cmd == 'carried':
        out.append('carried:null' if answer['carried'] is None else 'carried:entry')
    elif cmd == 'table':
        if answer.get('ok') is False:
            return ['table:NO_TABLE']
        out.append('table:read')
        out.append('home:null' if answer['home'] is None else 'home:set')
        out.append('lineage:root-alone' if len(answer['lineage']) == 1 else 'lineage:moved')
        out.append('homes:' + ('none' if not answer['homes'] else 'one' if len(answer['homes']) == 1 else 'several'))
        out.append('unread:zero' if answer['unread'] == 0 else 'unread:nonzero')
        out.append('unproven:zero' if answer['unproven'] == 0 else 'unproven:nonzero')
        out.append('persona:null' if answer['persona'] is None else 'persona:set')
        out.append('reroot:null' if answer['reroot'] is None else 'reroot:set')
        out.append('origin:null' if answer['origin'] is None else 'origin:set')
        for row in answer['faces']:
            out.append('state:' + row['state'])
            for sp in row['spans']:
                out.append('span:open' if sp['release'] is None else 'span:closed')
        for m in answer['misread']:
            out.append('reason:' + m['reason'])
    elif cmd == 'joined':
        if answer.get('ok') is False:
            return ['joined:NO_TABLE']
        out.append('joined:true' if answer['joined'] else 'joined:false')
    return out


def run(cmd, args):
    try:
        p = subprocess.run(cmd + args, capture_output=True, timeout=180, cwd=CORPUS)
    except subprocess.TimeoutExpired:
        return ('TIMEOUT', b'')
    if p.returncode != 0:
        return ('EXIT%d' % p.returncode, p.stdout[:300])
    return ('OK', p.stdout)


mpath = os.path.join(CORPUS, 'manifest.json')
mraw = open(mpath, 'rb').read()
m = json.loads(mraw)
print('=== manifest sha256 0x%s (seed %s)' % (hashlib.sha256(mraw).hexdigest(), m.get('seed')))
READ.update(b'manifest.json\0' + mraw + b'\0')
div, predmiss, summary = [], [], collections.Counter()
tally = collections.Counter()
cases = m.get('cases')
if not isinstance(cases, list) or not cases:
    sys.exit('the manifest names no cases')
seen_paths = set()
file_paths = set()
counts = collections.Counter()
answers = {}


def slug_of(path):
    m_ = re.match(r'^[a-z]+/[a-z]\d{3}_(.+?)(\.(bin|json))?$', path)
    return m_.group(1) if m_ else path
for c in cases:
    if not isinstance(c, dict) or set(c.keys()) != CASE_KEYS:
        predmiss.append((str(c)[:80], 'case', None, 'a case is an object with exactly path, category, cmd, args, note, predicted'))
        continue
    cat, path, args, pred = c['category'], c['path'], c['args'], c['predicted']
    counts[cat] += 1
    if cat not in CATEGORIES or (CATEGORIES[cat] is not None and c['cmd'] != CATEGORIES[cat]):
        predmiss.append((path, 'category', None, 'an unknown category, or a command that is not the category\'s'))
        continue
    if isinstance(args, list) and c['cmd'] != (args[0] if args else ''):
        predmiss.append((path, 'args', None, 'the case\'s command is not the first of its args'))
    if path in seen_paths:
        predmiss.append((path, 'path', None, 'a path named twice'))
    seen_paths.add(path)
    if not (isinstance(args, list) and all(isinstance(a, str) for a in args)):
        predmiss.append((path, 'args', None, 'args is an array of strings'))
        continue
    if cat in DIRS and not re.match(r'^%s/%s\d{3}_[A-Za-z0-9_-]+\.(bin|json)$' % (cat, cat[0]), path):
        predmiss.append((path, 'path', None, 'a file case\'s path is not in its category\'s directory and form'))
    if cat in DIRS:
        full = os.path.normpath(os.path.join(CORPUS, path))
        if not full.startswith(CORPUS + os.sep) or not os.path.isfile(full) or os.path.islink(full):
            predmiss.append((path, 'file', None, 'the case\'s file is not a regular file of the corpus'))
            continue
        if args[:2] != [c['cmd'], path]:
            predmiss.append((path, 'args', None, 'the case\'s args do not begin with its command and its path'))
        data_ = open(full, 'rb').read()
        READ.update(path.encode() + b'\0' + data_ + b'\0')
        file_paths.add(path)
    READ.update(json.dumps([path, args, pred], sort_keys=True).encode() + b'\0')
    if not (isinstance(pred, dict) and (pred == {'exit': 2} or (set(pred.keys()) == {'exit', 'stdout'}
            and pred['exit'] == 0 and isinstance(pred['stdout'], str)))):
        predmiss.append((path, 'predicted', None, 'a prediction is {"exit":2} or {"exit":0,"stdout":<string>}'))
        continue
    a = run(CAND, args)
    b = run(PY, args)
    summary[c['cmd'] or '(no command)'] += 1
    if a != b:
        div.append((c, a, b))
    if pred['exit'] == 2:
        got = (b[0], b[1])
        if got != ('EXIT2', b''):
            predmiss.append((path, 'exit 2, no output', got, c['note'][:80]))
        if cat != 'misuse':
            predmiss.append((path, 'category', None, 'only a misuse case predicts exit 2'))
        tally.update(labels_of(c['cmd'], None))
        if got == ('EXIT2', b''):
            answers.setdefault(slug_of(path), []).append((None, None))
    else:
        if cat == 'misuse':
            predmiss.append((path, 'category', None, 'a misuse case predicts exit 2'))
        if b != ('OK', pred['stdout'].encode('utf-8')):
            predmiss.append((path, pred['stdout'][:160], (b[0], b[1][:160]), c['note'][:80]))
        if b[0] == 'OK':
            try:
                inp_ = None
                if cat in ('tables', 'entries', 'joined'):
                    try:
                        inp_ = json.loads(data_)
                    except Exception:
                        pass
                answers.setdefault(slug_of(path), []).append((json.loads(b[1]), inp_))
                tally.update(labels_of(c['cmd'], json.loads(b[1])))
            except Exception as exc:
                predmiss.append((path, 'answer', None, 'the criterion\'s answer is not the object its command writes: %r' % (exc,)))
if dict(counts) != m.get('counts'):
    predmiss.append(('manifest', 'counts', dict(counts), 'the manifest\'s counts are not its cases'))
# a slug names one case per command, so that every name of the boundary list resolves to one case
_slugs = collections.Counter((c.get('cmd'), slug_of(str(c.get('path')))) for c in cases if isinstance(c, dict))
for (cmd_, slug_), n_ in sorted(_slugs.items(), key=str):
    if n_ > 1:
        predmiss.append((slug_, 'slug', cmd_, 'two cases of one command share a slug'))
# every entry of the corpus tree is a directory or a regular file, no link among them, and every
# file is a file case's path, the manifest, the README or the generator
for dp, dn, fn in os.walk(CORPUS):
    for n in dn + fn:
        full = os.path.join(dp, n)
        rel = os.path.relpath(full, CORPUS)
        if os.path.islink(full) or not (os.path.isdir(full) or os.path.isfile(full)):
            predmiss.append((rel, 'file', None, 'a link or a special file in the corpus tree'))
        elif os.path.isfile(full) and rel not in file_paths and rel not in ('manifest.json', 'README.md', 'gen.py'):
            predmiss.append((rel, 'file', None, 'a corpus file no file case names'))
# the boundary list (sec 10.1): the manifest's equals the comparator's, and every side is witnessed
if m.get('unobservable') != UNOBSERVABLE or not set(n for n, _ in UNOBSERVABLE) <= set(n for n, _, _ in B):
    predmiss.append(('manifest', 'unobservable', None, 'the manifest\'s unobservable boundaries are not the comparator\'s'))
if m.get('boundaries') != BOUNDARY_DATA:
    predmiss.append(('manifest', 'boundaries', None, 'the manifest\'s boundary list is not the comparator\'s'))
for name, sa, sb in B:
    for slug, test in sa + sb:
        got_ = answers.get(slug)
        ok_ = False
        for x, inp_ in got_ or []:
            try:
                ok_ = ok_ or bool(test.f(x, inp_) if isinstance(test, I) else test(x))
            except Exception:
                pass
        if not ok_:
            predmiss.append(('boundary', name, slug, 'no answer of the named case passes its side\'s test'))
if m.get('vocabularies') != VOCAB:
    predmiss.append(('manifest', 'vocabularies', None, 'the manifest\'s vocabularies are not the comparator\'s'))
if m.get('coverage') != dict(tally):
    predmiss.append(('manifest', 'coverage', None, 'the manifest\'s coverage differs from the criterion\'s measured tally'))
residue = sorted(lab for lab in VOCAB if not tally[lab])
if residue or m.get('unwitnessed') != residue:
    predmiss.append(('manifest', 'unwitnessed', residue, 'a label of the closed vocabularies has no witness, or the manifest\'s list is not the measured one'))
print('=== summary')
for k in sorted(summary):
    print('  %s %d' % (k, summary[k]))
print('=== bytes read sha256 0x%s' % READ.hexdigest())
if EXPECT is not None and EXPECT.lower() != '0x' + READ.hexdigest():
    predmiss.append(('corpus', EXPECT, '0x' + READ.hexdigest(), 'bytes read digest is not the expected one'))
print('=== divergences', len(div))
groups = collections.defaultdict(list)
for c, a, b in div:
    groups[(c['cmd'], a[0], a[1][:160], b[0], b[1][:160])].append(c['path'])
for key, paths in sorted(groups.items(), key=lambda kv: -len(kv[1])):
    print('\n--- %s x%d e.g. %s' % (key[0], len(paths), ', '.join(paths[:3])))
    print('  candidate:', key[1], key[2])
    print('  criterion:', key[3], key[4])
print('=== disagreements with prediction', len(predmiss))
for x in predmiss[:40]:
    print('  ', str(x)[:500])
json.dump([{'case': c['path'], 'candidate': [a[0], a[1].decode('utf-8', 'replace')],
            'criterion': [b[0], b[1].decode('utf-8', 'replace')]} for c, a, b in div], open(OUT, 'w'), indent=1)
sys.exit(1 if (div or predmiss) else 0)
