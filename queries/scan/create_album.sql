INSERT INTO album (title, title_sortable, artist_id, image, thumb, release_date, label, catalog_number, isrc, mbid)
    VALUES (
        $1,
        $2,
        NULLIF($3, 0),
        CASE WHEN length($4) = 0 THEN NULL ELSE $4 END,
        CASE WHEN length($5) = 0 THEN NULL ELSE $5 END,
        NULLIF($6, 0),
        NULLIF($7, ''),
        NULLIF($8, ''),
        NULLIF($9, ''),
        $10
    )
    ON CONFLICT (title, artist_id, mbid) DO UPDATE SET
        title_sortable = EXCLUDED.title_sortable,
        image = EXCLUDED.image,
        thumb = EXCLUDED.thumb,
        release_date = EXCLUDED.release_date,
        label = EXCLUDED.label,
        catalog_number = EXCLUDED.catalog_number,
        isrc = EXCLUDED.isrc;
