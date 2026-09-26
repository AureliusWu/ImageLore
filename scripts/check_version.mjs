import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const expected = fs.readFileSync(path.join(root,'VERSION'),'utf8').trim();
const pkg = JSON.parse(fs.readFileSync(path.join(root,'package.json'),'utf8')).version;
const tauri = JSON.parse(fs.readFileSync(path.join(root,'src-tauri','tauri.conf.json'),'utf8')).version;
const cargo = fs.readFileSync(path.join(root,'src-tauri','Cargo.toml'),'utf8').match(/\[package\][\s\S]*?version\s*=\s*"([^"]+)"/)?.[1];
const generated = fs.readFileSync(path.join(root,'src','version.ts'),'utf8').match(/APP_VERSION = "([^"]+)"/)?.[1];
const rows = {VERSION:expected,'package.json':pkg,'tauri.conf.json':tauri,'Cargo.toml':cargo,'UI generated':generated};
const bad = Object.entries(rows).filter(([,v])=>v!==expected);
console.table(rows);
if (bad.length) { console.error('Version mismatch:', bad); process.exit(1); }
console.log(`Version check PASS: ${expected}`);
