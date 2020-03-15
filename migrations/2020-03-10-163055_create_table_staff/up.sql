CREATE TYPE role AS ENUM ('janitor', 'moderator', 'administrator');

CREATE TABLE staff(
    name TEXT PRIMARY KEY,
    password_hash TEXT NOT NULL,
    role role NOT NULL);
