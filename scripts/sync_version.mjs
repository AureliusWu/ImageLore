import path from "node:path";
import { fileURLToPath } from "node:url";
import { readCanonicalVersion, syncVersionTargets } from "./version_targets.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const version = readCanonicalVersion(root);

syncVersionTargets(root, version);
console.log(`ImageLore version synced from VERSION: ${version}`);
