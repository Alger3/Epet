ALTER TABLE runtime_state ADD COLUMN autonomous_movement INTEGER NOT NULL DEFAULT 0
  CHECK (autonomous_movement IN (0, 1));

PRAGMA user_version = 5;
