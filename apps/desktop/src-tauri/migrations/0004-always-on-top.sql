ALTER TABLE runtime_state ADD COLUMN always_on_top INTEGER NOT NULL DEFAULT 1
  CHECK (always_on_top IN (0, 1));

PRAGMA user_version = 4;
