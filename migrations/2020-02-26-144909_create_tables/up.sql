CREATE TABLE board (
    name TEXT PRIMARY KEY,
    description TEXT NOT NULL);

CREATE TABLE thread (
    id SERIAL PRIMARY KEY,
    time_stamp TIMESTAMPTZ NOT NULL,
    subject TEXT NOT NULL,
    board TEXT NOT NULL REFERENCES board);

CREATE TABLE post (
    id SERIAL PRIMARY KEY,
    time_stamp TIMESTAMPTZ NOT NULL,
    body TEXT NOT NULL,
    author_name TEXT,
    author_contact TEXT,
    author_ident TEXT,
    thread INTEGER NOT NULL REFERENCES thread);
