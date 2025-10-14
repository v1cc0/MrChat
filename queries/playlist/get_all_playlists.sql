SELECT
    playlist.id,
    playlist.name,
    playlist.created_at,
    playlist.type AS playlist_type,
    COUNT(playlist_item.id) AS track_count
FROM playlist
LEFT JOIN playlist_item ON playlist.id = playlist_item.playlist_id
GROUP BY
    playlist.id,
    playlist.name,
    playlist.created_at,
    playlist.type
ORDER BY playlist.created_at DESC;
