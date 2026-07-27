ALTER TABLE runtime_state ADD COLUMN edge_dock TEXT
  CHECK (edge_dock IS NULL OR edge_dock IN ('left', 'right', 'top', 'bottom'));

PRAGMA user_version = 9;
