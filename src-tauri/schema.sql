PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS app_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
INSERT OR IGNORE INTO app_meta(key,value) VALUES ('schema_version','1');

CREATE TABLE IF NOT EXISTS assets (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0,1)),
  width INTEGER,
  height INTEGER,
  file_size INTEGER,
  format TEXT NOT NULL DEFAULT '',
  mime_type TEXT NOT NULL DEFAULT '',
  metadata_type TEXT NOT NULL DEFAULT 'none',
  generation_json TEXT NOT NULL DEFAULT '{}',
  fingerprint TEXT NOT NULL DEFAULT '',
  file_mtime INTEGER NOT NULL DEFAULT 0,
  missing INTEGER NOT NULL DEFAULT 0 CHECK (missing IN (0,1)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_assets_updated ON assets(updated_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_assets_favorite ON assets(favorite, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_assets_missing ON assets(missing, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_assets_fingerprint ON assets(fingerprint);

CREATE TABLE IF NOT EXISTS prompt_state (
  asset_id INTEGER PRIMARY KEY,
  prompt TEXT NOT NULL DEFAULT '',
  negative_prompt TEXT NOT NULL DEFAULT '',
  model TEXT NOT NULL DEFAULT '',
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tags (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS asset_tags (
  asset_id INTEGER NOT NULL,
  tag_id INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(asset_id, tag_id),
  FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE,
  FOREIGN KEY(tag_id) REFERENCES tags(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_asset_tags_tag ON asset_tags(tag_id, asset_id);

CREATE TABLE IF NOT EXISTS prompt_revisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  asset_id INTEGER NOT NULL,
  prompt TEXT NOT NULL DEFAULT '',
  negative_prompt TEXT NOT NULL DEFAULT '',
  model TEXT NOT NULL DEFAULT '',
  tags_json TEXT NOT NULL DEFAULT '[]',
  note TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_revisions_asset ON prompt_revisions(asset_id, created_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS relations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  parent_id INTEGER NOT NULL,
  child_id INTEGER NOT NULL,
  relation_type TEXT NOT NULL DEFAULT 'derived_from',
  note TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  UNIQUE(parent_id, child_id, relation_type),
  FOREIGN KEY(parent_id) REFERENCES assets(id) ON DELETE CASCADE,
  FOREIGN KEY(child_id) REFERENCES assets(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_relations_parent ON relations(parent_id);
CREATE INDEX IF NOT EXISTS idx_relations_child ON relations(child_id);

CREATE TABLE IF NOT EXISTS collections (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  description TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS collection_assets (
  collection_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(collection_id, asset_id),
  FOREIGN KEY(collection_id) REFERENCES collections(id) ON DELETE CASCADE,
  FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_collection_assets_asset ON collection_assets(asset_id, collection_id);

CREATE VIRTUAL TABLE IF NOT EXISTS asset_search USING fts5(
  asset_id UNINDEXED,
  name,
  prompt,
  negative_prompt,
  model,
  tags,
  tokenize='unicode61 remove_diacritics 2'
);
