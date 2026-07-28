from contextlib import contextmanager

import psycopg
from psycopg.rows import dict_row

from .settings import settings


SCHEMA = """
CREATE TABLE IF NOT EXISTS uploads (
  id TEXT PRIMARY KEY,
  role TEXT NOT NULL,
  size_bytes BIGINT NOT NULL,
  mime_type TEXT NOT NULL,
  sha256 TEXT NOT NULL,
  object_key TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  error_code TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS generation_jobs (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  primary_upload_id TEXT NOT NULL REFERENCES uploads(id),
  additional_upload_ids JSONB NOT NULL DEFAULT '[]',
  style_id TEXT NOT NULL,
  species TEXT NOT NULL,
  subject_kind TEXT NOT NULL DEFAULT 'pet_cat',
  stage TEXT NOT NULL,
  progress DOUBLE PRECISION,
  retryable BOOLEAN NOT NULL DEFAULT FALSE,
  version INTEGER NOT NULL DEFAULT 1,
  error_code TEXT,
  error_params JSONB NOT NULL DEFAULT '{}',
  artifact_key TEXT,
  artifact_sha256 TEXT,
  artifact_size BIGINT,
  provider_mode TEXT NOT NULL DEFAULT 'configured',
  requested_provider TEXT,
  requested_device_id TEXT,
  actual_provider TEXT,
  actual_device_id TEXT,
  model_id TEXT,
  estimated_speed TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE generation_jobs
  ADD COLUMN IF NOT EXISTS subject_kind TEXT NOT NULL DEFAULT 'pet_cat';
ALTER TABLE generation_jobs ADD COLUMN IF NOT EXISTS provider_mode TEXT NOT NULL DEFAULT 'configured';
ALTER TABLE generation_jobs ADD COLUMN IF NOT EXISTS requested_provider TEXT;
ALTER TABLE generation_jobs ADD COLUMN IF NOT EXISTS requested_device_id TEXT;
ALTER TABLE generation_jobs ADD COLUMN IF NOT EXISTS actual_provider TEXT;
ALTER TABLE generation_jobs ADD COLUMN IF NOT EXISTS actual_device_id TEXT;
ALTER TABLE generation_jobs ADD COLUMN IF NOT EXISTS model_id TEXT;
ALTER TABLE generation_jobs ADD COLUMN IF NOT EXISTS estimated_speed TEXT;

CREATE TABLE IF NOT EXISTS deletions (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ
);
"""


@contextmanager
def connection():
    with psycopg.connect(settings.database_url, row_factory=dict_row) as conn:
        yield conn


def initialize_database() -> None:
    with connection() as conn:
        conn.execute(SCHEMA)
        conn.commit()
