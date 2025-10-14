SELECT
    album_id,
    IFNULL(disc_number, -1) AS disc_key,
    folder
FROM track
WHERE location = $1;
