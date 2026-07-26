ALTER TABLE characters ADD COLUMN current_package_sha256 TEXT;
ALTER TABLE characters ADD COLUMN updated_at TEXT;

UPDATE characters
SET updated_at = created_at
WHERE updated_at IS NULL;

CREATE TABLE character_versions (
  character_id TEXT NOT NULL,
  package_version TEXT NOT NULL,
  package_sha256 TEXT NOT NULL,
  storage_key TEXT NOT NULL UNIQUE,
  package_size INTEGER NOT NULL CHECK (package_size > 0),
  source_url TEXT,
  installed_at TEXT NOT NULL,
  PRIMARY KEY (character_id, package_sha256),
  UNIQUE (character_id, package_version),
  FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE INDEX character_versions_character_id
ON character_versions(character_id);

PRAGMA user_version = 7;
