ALTER TABLE runtime_state ADD COLUMN work_area_width REAL;
ALTER TABLE runtime_state ADD COLUMN work_area_height REAL;
ALTER TABLE runtime_state ADD COLUMN dpi_scale REAL;
ALTER TABLE runtime_state ADD COLUMN pet_logical_size REAL NOT NULL DEFAULT 320.0;
ALTER TABLE runtime_state ADD COLUMN foot_anchor_x REAL;
ALTER TABLE runtime_state ADD COLUMN foot_anchor_y REAL;

PRAGMA user_version = 2;
