import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const kind = process.argv[2] || "patch";
if (!["patch", "minor", "major"].includes(kind)) throw new Error("Use patch, minor, or major");
const versionPath = path.join(root, "VERSION");
const current = fs.readFileSync(versionPath, "utf8").trim();
const m = current.match(/^(\d+)\.(\d+)\.(\d+)/);
if (!m) throw new Error(`Invalid current version: ${current}`);
let major = Number(m[1]),
  minor = Number(m[2]),
  patch = Number(m[3]);
if (kind === "patch") patch += 1;
if (kind === "minor") {
  minor += 1;
  patch = 0;
}
if (kind === "major") {
  major += 1;
  minor = 0;
  patch = 0;
}
const next = `${major}.${minor}.${patch}`;
fs.writeFileSync(versionPath, next + "\n");
const result = spawnSync(process.execPath, [path.join(__dirname, "sync_version.mjs")], {
  stdio: "inherit",
});
if (result.status !== 0) process.exit(result.status ?? 1);
console.log(`${current} -> ${next}`);
