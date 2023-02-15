-- Add migration script here
CREATE TABLE IF NOT EXISTS windows_log (
    datetime TEXT NOT NULL PRIMARY KEY,
    class_left TEXT NOT NULL,
    class_right TEXT NOT NULL,
    title TEXT NOT NULL,
    duration REAL NOT NULL
);
