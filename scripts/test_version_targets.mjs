import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  collectVersionEntries,
  readCanonicalVersion,
  syncVersionTargets,
} from "./version_targets.mjs";

const source = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "imagelore-version-test-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const name of [
    "VERSION",
    "package.json",
    "package-lock.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
    "src/version.ts",
    "README.md",
    "docs/UI_PREVIEW.html",
    "docs/og.svg",
  ]) {
    fs.mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
    fs.copyFileSync(path.join(source, name), path.join(root, name));
  }
  return root;
}
test("synchronization remains idempotent for the canonical version", (t) => {
  const root = fixture(t),
    version = readCanonicalVersion(root);
  syncVersionTargets(root, version);
  assert.ok(collectVersionEntries(root).entries.every(({ value }) => value === version));
  const before = fs.readFileSync(path.join(root, "README.md"), "utf8");
  syncVersionTargets(root, version);
  assert.equal(fs.readFileSync(path.join(root, "README.md"), "utf8"), before);
});
test("both checks and synchronization reject duplicated README version headings", (t) => {
  const root = fixture(t),
    version = readCanonicalVersion(root);
  fs.appendFileSync(path.join(root, "README.md"), `\n# ImageLore v${version}\n`);
  const heading = collectVersionEntries(root).entries.find(
    ({ label }) => label === "README.md heading",
  );
  assert.equal(heading.value, "marker-count:2");
  assert.throws(
    () => syncVersionTargets(root, version),
    /README current version heading: expected 1 version marker\(s\), found 2/,
  );
});
test("non-global Cargo.lock targets count every matching package marker", (t) => {
  const root = fixture(t),
    version = readCanonicalVersion(root);
  fs.appendFileSync(
    path.join(root, "src-tauri/Cargo.lock"),
    `\n[[package]]\nname = "imagelore"\nversion = "0.0.0"\n`,
  );
  assert.throws(
    () => syncVersionTargets(root, version),
    /Cargo.lock ImageLore version: expected 1 version marker\(s\), found 2/,
  );
});
