SELECT
    id,
    title_sortable
FROM
    (
        SELECT
            id,
            title_sortable,
            catalog_number,
            release_date
        FROM
            album
        ORDER BY
            catalog_number COLLATE NOCASE ASC,
            release_date ASC
    );
