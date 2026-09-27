import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { collectVersionEntries } from './version_targets.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const { expected, entries } = collectVersionEntries(root);
const rows = Object.fromEntries(entries.map(({ label, value }) => [label, value]));
const bad = entries.filter(({ value }) => value !== expected);

console.table(rows);
if (bad.length) {
  console.error('Version mismatch:', bad);
  process.exit(1);
}
console.log(`Version check PASS: every current-version target matches VERSION (${expected})`);
