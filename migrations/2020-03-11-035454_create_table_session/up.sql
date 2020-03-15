CREATE TABLE session (
    id TEXT PRIMARY KEY,
    expires TIMESTAMPTZ NOT NULL,
    staff_name TEXT NOT NULL REFERENCES staff);
