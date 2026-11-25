#!/usr/bin/env node
// Reads the barred table of zikaron.face/1 section 1 out of the law's text and sets it beside the
// Unicode data of the runtime (node's ICU): the classes the law's sentence names, and the values
// the table bars beside them.  An instrument of the workbench, sealed by nothing; it prints what
// it finds and exits 0 where the sentence holds for this runtime's Unicode version.
// usage: node face-table-unicode.js [path to zikaron-face-v1.md]
'use strict';
const fs = require('fs'), path = require('path');
const law = process.argv[2] || path.join(__dirname, '..', 'docs', 'zikaron-face-v1.md');
const rows = [...fs.readFileSync(law, 'utf8').matchAll(/^\| U\+([0-9A-F]+) \| U\+([0-9A-F]+) \|/gm)]
  .map(m => [parseInt(m[1], 16), parseInt(m[2], 16)]);
const barred = o => (o & 0xFFFE) === 0xFFFE || rows.some(([a, b]) => a <= o && o <= b);
const cls = /[\p{Cc}\p{Cf}\p{Zs}\p{Zl}\p{Zp}\p{Co}\p{White_Space}\p{Default_Ignorable_Code_Point}\p{Noncharacter_Code_Point}]/u;
const free = [], beside = [];
for (let o = 0; o < 0x110000; o++) {
  if (o >= 0xD800 && o <= 0xDFFF) continue;
  const inClass = cls.test(String.fromCodePoint(o));
  if (inClass && !barred(o)) free.push(o);
  if (!inClass && barred(o)) beside.push(o);
}
const u = o => 'U+' + o.toString(16).toUpperCase().padStart(4, '0');
console.log('rows of the table: ' + rows.length);
console.log('Unicode version of this runtime: ' + process.versions.unicode);
console.log('values of the named classes left free: ' + free.length + (free.length ? ' ' + free.map(u).join(' ') : ''));
console.log('values barred beside the named classes: ' + beside.length + ' ' + beside.map(u).join(' '));
const named = [0x2800, 0xFFFC, 0xFFFD, 0x13441, 0x13442, 0x16FE4, 0x1BC9D, 0x1D159];
const ok = free.length === 0 && beside.length === named.length && beside.every((o, i) => o === named[i]);
console.log(ok ? 'the sentence of section 1 holds for this version' : 'the sentence of section 1 does not hold for this version');
process.exit(ok ? 0 : 1);
