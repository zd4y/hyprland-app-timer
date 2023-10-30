-- Add migration script here
CREATE TABLE IF NOT EXISTS windows_log (
    datetime TEXT NOT NULL PRIMARY KEY,
    class TEXT NOT NULL,
    title TEXT NOT NULL,
    duration REAL NOT NULL
);
