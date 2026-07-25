ALTER TABLE runtime_state ADD COLUMN sleep_after_minutes INTEGER NOT NULL DEFAULT 10
  CHECK (sleep_after_minutes IN (0, 1, 5, 10, 20, 30));

PRAGMA user_version = 6;
