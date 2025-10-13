SELECT
    id,
    title_sortable
FROM
    (
        SELECT
            id,
            title_sortable,
            label,
            catalog_number,
            release_date
        FROM
            album
        ORDER BY
            label COLLATE NOCASE ASC,
            catalog_number COLLATE NOCASE ASC,
            release_date ASC
    );
