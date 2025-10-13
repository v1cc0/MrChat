SELECT
    id,
    title_sortable
FROM
    (
        SELECT
            p.id,
            p.title_sortable,
            p.release_date,
            a.name_sortable
        FROM
            album p
            JOIN artist a ON p.artist_id = a.id
        ORDER BY
            a.name_sortable COLLATE NOCASE DESC,
            p.release_date ASC
    );
