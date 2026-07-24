CREATE TABLE IF NOT EXISTS characters (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  subject_kind TEXT NOT NULL CHECK (subject_kind IN ('pet_cat', 'human_avatar')),
  asset_key TEXT NOT NULL UNIQUE,
  built_in INTEGER NOT NULL CHECK (built_in IN (0, 1)),
  created_at TEXT NOT NULL
);

INSERT OR IGNORE INTO characters (id, name, subject_kind, asset_key, built_in, created_at)
VALUES
  ('builtin-orange-tabby', '橘子', 'pet_cat', 'builtin-orange-tabby', 1, '2026-07-24T00:00:00Z'),
  ('builtin-forest-guide', '小栎', 'human_avatar', 'builtin-forest-guide', 1, '2026-07-24T00:00:00Z');

ALTER TABLE runtime_state ADD COLUMN active_character_id TEXT;

UPDATE runtime_state
SET active_character_id = COALESCE(active_pet_id, 'builtin-orange-tabby')
WHERE active_character_id IS NULL;

PRAGMA user_version = 3;
