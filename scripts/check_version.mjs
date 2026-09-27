import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const expected = fs.readFileSync(path.join(root,'VERSION'),'utf8').trim();

const pkg = JSON.parse(fs.readFileSync(path.join(root,'package.json'),'utf8')).version;
const npmLockJson = JSON.parse(fs.readFileSync(path.join(root,'package-lock.json'),'utf8'));
const npmLock = npmLockJson.packages?.['']?.version ?? npmLockJson.version;
const tauri = JSON.parse(fs.readFileSync(path.join(root,'src-tauri','tauri.conf.json'),'utf8')).version;
const cargoText = fs.readFileSync(path.join(root,'src-tauri','Cargo.toml'),'utf8');
const cargo = cargoText.match(/\[package\][\s\S]*?version\s*=\s*"([^"]+)"/)?.[1];
const cargoLockText = fs.readFileSync(path.join(root,'src-tauri','Cargo.lock'),'utf8');
const cargoLock = cargoLockText.match(/\[\[package\]\]\r?\nname = "imagelore"\r?\nversion = "([^"]+)"/)?.[1];
const generated = fs.readFileSync(path.join(root,'src','version.ts'),'utf8').match(/APP_VERSION = "([^"]+)"/)?.[1];

const rows = {
  VERSION: expected,
  'package.json': pkg,
  'package-lock.json': npmLock,
  'tauri.conf.json': tauri,
  'Cargo.toml': cargo,
  'Cargo.lock': cargoLock,
  'UI generated': generated
};
const bad = Object.entries(rows).filter(([,v])=>v!==expected);
console.table(rows);
if (bad.length) {
  console.error('Version mismatch:', bad);
  process.exit(1);
}
console.log(`Version check PASS: ${expected}`);
