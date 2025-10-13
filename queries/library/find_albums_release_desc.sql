SELECT
    id,
    title_sortable
FROM
    (
        SELECT
            id,
            title_sortable,
            release_date
        FROM
            album
        ORDER BY
            release_date DESC,
            title_sortable COLLATE NOCASE ASC
    );
