ALTER TABLE characters ADD COLUMN custom_name TEXT;
ALTER TABLE characters ADD COLUMN cloud_deletion_status TEXT NOT NULL DEFAULT 'not_requested'
  CHECK (cloud_deletion_status IN ('not_requested', 'requested', 'processing', 'completed', 'failed'));

CREATE TABLE creation_drafts (
  id TEXT PRIMARY KEY,
  subject_kind TEXT NOT NULL CHECK (subject_kind IN ('pet_cat', 'human_avatar')),
  display_name TEXT,
  authorization_confirmed INTEGER NOT NULL DEFAULT 0 CHECK (authorization_confirmed IN (0, 1)),
  authorization_version TEXT,
  status TEXT NOT NULL CHECK (status IN (
    'editing', 'ready', 'submitting', 'checking', 'queued',
    'generating_portrait', 'awaiting_confirmation', 'generating_actions',
    'packaging', 'completed', 'service_unavailable', 'failed', 'cancelled'
  )),
  snapshot_version INTEGER NOT NULL DEFAULT 0 CHECK (snapshot_version >= 0),
  progress_percent INTEGER CHECK (progress_percent BETWEEN 0 AND 100),
  server_job_id TEXT,
  server_expires_at TEXT,
  error_code TEXT,
  error_message TEXT,
  retryable INTEGER NOT NULL DEFAULT 0 CHECK (retryable IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX creation_drafts_updated_at ON creation_drafts(updated_at DESC);

CREATE TABLE draft_photos (
  draft_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('primary', 'supplemental_1', 'supplemental_2')),
  original_name TEXT NOT NULL,
  storage_key TEXT NOT NULL,
  mime_type TEXT NOT NULL CHECK (mime_type IN ('image/jpeg', 'image/png')),
  width INTEGER NOT NULL CHECK (width > 0),
  height INTEGER NOT NULL CHECK (height > 0),
  byte_size INTEGER NOT NULL CHECK (byte_size > 0),
  sha256 TEXT NOT NULL,
  crop_x REAL NOT NULL CHECK (crop_x BETWEEN 0 AND 1),
  crop_y REAL NOT NULL CHECK (crop_y BETWEEN 0 AND 1),
  crop_width REAL NOT NULL CHECK (crop_width > 0 AND crop_width <= 1),
  crop_height REAL NOT NULL CHECK (crop_height > 0 AND crop_height <= 1),
  quality_status TEXT NOT NULL CHECK (quality_status IN ('accepted', 'warning')),
  quality_messages TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (draft_id, role),
  FOREIGN KEY (draft_id) REFERENCES creation_drafts(id) ON DELETE CASCADE
);

PRAGMA user_version = 8;
