-- Add MusicBrainz ID to album table
-- Default to 'none' for albums without MBID
ALTER TABLE album ADD COLUMN mbid TEXT DEFAULT 'none' NOT NULL;

-- Create album_path table for tracking album disc locations
CREATE TABLE IF NOT EXISTS album_path (
    album_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    disc_num INTEGER DEFAULT -1 NOT NULL,
    FOREIGN KEY (album_id) REFERENCES album (id),
    PRIMARY KEY (album_id, disc_num)
);

-- Add new unique index that includes mbid
-- Note: libSQL does not permit dropping the earlier UNIQUE index;
-- keep album_title_artist_id_idx and add the new one alongside
CREATE UNIQUE INDEX IF NOT EXISTS album_title_artist_mbid ON album (title, artist_id, mbid);
