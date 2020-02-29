CREATE TABLE file (
    save_name TEXT PRIMARY KEY,
    thumb_name TEXT,
    orig_name TEXT,
    content_type TEXT,
    post INTEGER NOT NULL REFERENCES post);
