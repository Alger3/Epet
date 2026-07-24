CREATE TABLE IF NOT EXISTS runtime_state (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  active_pet_id TEXT NOT NULL,
  monitor_id TEXT,
  x REAL,
  y REAL,
  scale REAL NOT NULL CHECK (scale BETWEEN 0.5 AND 1.5),
  visible INTEGER NOT NULL CHECK (visible IN (0, 1)),
  click_through INTEGER NOT NULL CHECK (click_through IN (0, 1)),
  paused INTEGER NOT NULL CHECK (paused IN (0, 1)),
  last_behavior_state TEXT NOT NULL,
  runtime_version INTEGER NOT NULL
);

PRAGMA user_version = 1;
