-- just set this to 'none' if there isn't one for now
ALTER TABLE album ADD mbid TEXT DEFAULT 'none' NOT NULL;

CREATE TABLE IF NOT EXISTS album_path (
    album_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    disc_num INTEGER DEFAULT -1 NOT NULL,
    FOREIGN KEY (album_id) REFERENCES album (id),
    PRIMARY KEY (album_id, disc_num)
);

-- libSQL does not permit dropping the earlier UNIQUE index; keep it and add the new one alongside.
CREATE UNIQUE INDEX IF NOT EXISTS album_title_artist_mbid ON album (title, artist_id, mbid);

-- Trigger removed: libSQL/Turso does not support CREATE TRIGGER.
