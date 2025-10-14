SELECT COUNT(*) AS track_count, COALESCE(SUM(duration), 0) AS total_duration FROM track;
