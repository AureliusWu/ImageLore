# Historical migration fixtures

These are unmodified schema SQL sources frozen at the full commits and SHA-256 hashes in manifest.json. The schema-2 fixture also freezes the real historical Rust migration implementation. The Rust matrix opens a fresh temporary file database, executes each frozen source (schema 2 runs its frozen apply function), inserts deterministic synthetic business sentinels available in that version, migrates to the current schema twice, and compares business data and foreign keys. No normal user library is opened.

Regenerate a source with its exact manifest command; do not adjust old schema markers or derive old versions by deleting tables from the current schema.

Run the frozen-source matrix with cargo test --locked --manifest-path src-tauri/Cargo.toml --lib migrations::tests.

Synthetic data summary: every version contains 2 assets and Prompt states, 1 revision, 1 tag and tag membership, 1 collection and collection membership, and 1 parent-child relation. Schema 3 adds explicit stable portable IDs, 1 Session and membership, 1 alias, 1 saved filter and 1 unresolved portable relation. Schema 4 adds 1 source folder. Schema 5 adds 2 derived generation-index rows. Schema 6 adds 1 embedding and preserves semantic settings. Schema 8 adds 1 manual DNA row. Schema 9 adds 1 analysis and configured model. Schema 10 adds 1 Remix draft and multi-asset source relation. Schema 11 adds 1 Reference source with synthetic metadata. Tests compare full ordered business rows, not only counts; new portable IDs for schema 1/2 must match the deterministic original algorithm, and repeated migration must keep them unchanged.
