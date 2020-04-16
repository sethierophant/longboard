CREATE TABLE staff_action (
    id SERIAL PRIMARY KEY,
    done_by TEXT NOT NULL REFERENCES staff,
    action TEXT NOT NULL,
    reason TEXT NOT NULL,
    time_stamp TIMESTAMPTZ NOT NULL DEFAULT NOW());
