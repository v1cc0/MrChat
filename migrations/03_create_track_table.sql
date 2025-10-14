-- Track table: stores individual music tracks
CREATE TABLE IF NOT EXISTS track (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    title_sortable TEXT NOT NULL,
    album_id INTEGER,
    track_number INTEGER,
    disc_number INTEGER,
    duration INTEGER NOT NULL,  -- Duration in seconds
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    genres TEXT,
    tags TEXT,
    location TEXT NOT NULL UNIQUE,  -- File path
    FOREIGN KEY (album_id) REFERENCES album (id)
);
