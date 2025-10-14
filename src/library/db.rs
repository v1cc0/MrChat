use std::sync::Arc;

use anyhow::Result;
use gpui::App;
use smol::block_on;
use tracing::debug;

use crate::{
    db::TursoDatabase,
    library::types::{Playlist, PlaylistItem, PlaylistWithCount, TrackStats},
    ui::app::Pool,
};

use super::types::{Album, Artist, Track};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlbumSortMethod {
    TitleAsc,
    TitleDesc,
    ArtistAsc,
    ArtistDesc,
    ReleaseAsc,
    ReleaseDesc,
    LabelAsc,
    LabelDesc,
    CatalogAsc,
    CatalogDesc,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlbumMethod {
    FullQuality,
    Thumbnail,
}

pub async fn list_albums(
    db: &TursoDatabase,
    sort_method: AlbumSortMethod,
) -> Result<Vec<(u32, String)>> {
    let query = match sort_method {
        AlbumSortMethod::TitleAsc => {
            include_str!("../../queries/library/find_albums_title_asc.sql")
        }
        AlbumSortMethod::TitleDesc => {
            include_str!("../../queries/library/find_albums_title_desc.sql")
        }
        AlbumSortMethod::ArtistAsc => {
            include_str!("../../queries/library/find_albums_artist_asc.sql")
        }
        AlbumSortMethod::ArtistDesc => {
            include_str!("../../queries/library/find_albums_artist_desc.sql")
        }
        AlbumSortMethod::ReleaseAsc => {
            include_str!("../../queries/library/find_albums_release_asc.sql")
        }
        AlbumSortMethod::ReleaseDesc => {
            include_str!("../../queries/library/find_albums_release_desc.sql")
        }
        AlbumSortMethod::LabelAsc => {
            include_str!("../../queries/library/find_albums_label_asc.sql")
        }
        AlbumSortMethod::LabelDesc => {
            include_str!("../../queries/library/find_albums_label_desc.sql")
        }
        AlbumSortMethod::CatalogAsc => {
            include_str!("../../queries/library/find_albums_catnum_asc.sql")
        }
        AlbumSortMethod::CatalogDesc => {
            include_str!("../../queries/library/find_albums_catnum_desc.sql")
        }
    };

    let conn = db.connect()?;
    conn.query_map(query, (), |row| {
        Ok((row.get::<i64>(0)? as u32, row.get::<String>(1)?))
    })
    .await
}

pub async fn list_tracks_in_album(db: &TursoDatabase, album_id: i64) -> Result<Arc<Vec<Track>>> {
    let query = include_str!("../../queries/library/find_tracks_in_album.sql");

    let conn = db.connect()?;
    let tracks = conn.query_map(query, [album_id], Track::from_row).await?;

    Ok(Arc::new(tracks))
}

pub async fn get_album_by_id(
    db: &TursoDatabase,
    album_id: i64,
    method: AlbumMethod,
) -> Result<Arc<Album>> {
    let query = include_str!("../../queries/library/find_album_by_id.sql");

    let conn = db.connect()?;
    let mut album = conn.query_one(query, [album_id], Album::from_row).await?;

    match method {
        AlbumMethod::FullQuality => {
            album.thumb = None;
        }
        AlbumMethod::Thumbnail => {
            album.image = None;
        }
    }

    Ok(Arc::new(album))
}

pub async fn get_artist_name_by_id(db: &TursoDatabase, artist_id: i64) -> Result<Arc<String>> {
    let query = include_str!("../../queries/library/find_artist_name_by_id.sql");

    let conn = db.connect()?;
    let name: String = conn.query_scalar(query, [artist_id]).await?;

    Ok(Arc::new(name))
}

pub async fn get_artist_by_id(db: &TursoDatabase, artist_id: i64) -> Result<Arc<Artist>> {
    let query = include_str!("../../queries/library/find_artist_by_id.sql");

    let conn = db.connect()?;
    let artist = conn.query_one(query, [artist_id], Artist::from_row).await?;

    Ok(Arc::new(artist))
}

pub async fn get_track_by_id(db: &TursoDatabase, track_id: i64) -> Result<Arc<Track>> {
    let query = include_str!("../../queries/library/find_track_by_id.sql");

    let conn = db.connect()?;
    let track = conn.query_one(query, [track_id], Track::from_row).await?;

    Ok(Arc::new(track))
}

/// Lists all albums for searching. Returns a vector of tuples containing the id, name, and artist
/// name.
pub async fn list_albums_search(db: &TursoDatabase) -> Result<Vec<(u32, String, String)>> {
    let query = include_str!("../../queries/library/find_albums_search.sql");

    let conn = db.connect()?;
    conn.query_map(query, (), |row| {
        Ok((
            row.get::<i64>(0)? as u32,
            row.get::<String>(1)?,
            row.get::<String>(2)?,
        ))
    })
    .await
}

pub async fn add_playlist_item(db: &TursoDatabase, playlist_id: i64, track_id: i64) -> Result<i64> {
    let query = include_str!("../../queries/playlist/add_track.sql");

    let conn = db.connect()?;
    conn.execute_returning_id(query, [playlist_id, track_id])
        .await
}

pub async fn create_playlist(db: &TursoDatabase, name: &str) -> Result<i64> {
    let query = include_str!("../../queries/playlist/create_playlist.sql");

    let conn = db.connect()?;
    conn.execute_returning_id(query, [name]).await
}

pub async fn delete_playlist(db: &TursoDatabase, playlist_id: i64) -> Result<()> {
    let query = include_str!("../../queries/playlist/delete_playlist.sql");

    let conn = db.connect()?;
    conn.execute(query, [playlist_id]).await?;

    Ok(())
}

pub async fn get_all_playlists(db: &TursoDatabase) -> Result<Arc<Vec<PlaylistWithCount>>> {
    let query = include_str!("../../queries/playlist/get_all_playlists.sql");

    let conn = db.connect()?;
    let playlists = conn
        .query_map(query, (), PlaylistWithCount::from_row)
        .await?;

    Ok(Arc::new(playlists))
}

pub async fn get_playlist(db: &TursoDatabase, playlist_id: i64) -> Result<Arc<Playlist>> {
    let query = include_str!("../../queries/playlist/get_playlist.sql");

    let conn = db.connect()?;
    let playlist = conn
        .query_one(query, [playlist_id], Playlist::from_row)
        .await?;

    Ok(Arc::new(playlist))
}

pub async fn get_playlist_track_files(
    db: &TursoDatabase,
    playlist_id: i64,
) -> Result<Arc<Vec<String>>> {
    let query = include_str!("../../queries/playlist/get_track_files.sql");

    let conn = db.connect()?;
    let track_files = conn
        .query_map(query, [playlist_id], |row| Ok(row.get::<String>(0)?))
        .await?;

    Ok(Arc::new(track_files))
}

/// Returns (playlist_item_id, track_id, album_id)
pub async fn get_playlist_tracks(
    db: &TursoDatabase,
    playlist_id: i64,
) -> Result<Arc<Vec<(i64, i64, i64)>>> {
    let query = include_str!("../../queries/playlist/get_track_listing.sql");

    let conn = db.connect()?;
    let tracks = conn
        .query_map(query, [playlist_id], |row| {
            Ok((row.get::<i64>(0)?, row.get::<i64>(1)?, row.get::<i64>(2)?))
        })
        .await?;

    Ok(Arc::new(tracks))
}

pub async fn move_playlist_item(db: &TursoDatabase, item_id: i64, new_position: i64) -> Result<()> {
    // retrieve the current item's position
    let original_item = get_playlist_item(db, item_id).await?;

    if original_item.position > new_position {
        let move_query = include_str!("../../queries/playlist/move_track_down.sql");

        let conn = db.connect()?;
        conn.execute(move_query, [new_position, original_item.position, item_id])
            .await?;
    } else if original_item.position < new_position {
        let move_query = include_str!("../../queries/playlist/move_track_up.sql");

        let conn = db.connect()?;
        conn.execute(move_query, [new_position, original_item.position, item_id])
            .await?;
    }

    Ok(())
}

pub async fn remove_playlist_item(db: &TursoDatabase, item_id: i64) -> Result<()> {
    let query = include_str!("../../queries/playlist/remove_track.sql");
    let item = get_playlist_item(db, item_id).await?;

    let conn = db.connect()?;
    conn.execute(query, [item.position, item_id]).await?;

    Ok(())
}

pub async fn get_playlist_item(db: &TursoDatabase, item_id: i64) -> Result<PlaylistItem> {
    let query = include_str!("../../queries/playlist/select_playlist_item.sql");

    let conn = db.connect()?;
    conn.query_one(query, [item_id], PlaylistItem::from_row)
        .await
}

pub async fn get_track_stats(db: &TursoDatabase) -> Result<Arc<TrackStats>> {
    let query = include_str!("../../queries/track_stats.sql");

    let conn = db.connect()?;
    let stats = conn.query_one(query, (), TrackStats::from_row).await?;

    Ok(Arc::new(stats))
}

pub async fn playlist_has_track(
    db: &TursoDatabase,
    playlist_id: i64,
    track_id: i64,
) -> Result<Option<i64>> {
    let query = include_str!("../../queries/playlist/playlist_has_track.sql");

    let conn = db.connect()?;
    conn.query_scalar_optional(query, [playlist_id, track_id])
        .await
}

pub trait LibraryAccess {
    fn list_albums(&self, sort_method: AlbumSortMethod) -> Result<Vec<(u32, String)>>;
    fn list_tracks_in_album(&self, album_id: i64) -> Result<Arc<Vec<Track>>>;
    fn get_album_by_id(&self, album_id: i64, method: AlbumMethod) -> Result<Arc<Album>>;
    fn get_artist_name_by_id(&self, artist_id: i64) -> Result<Arc<String>>;
    fn get_artist_by_id(&self, artist_id: i64) -> Result<Arc<Artist>>;
    fn get_track_by_id(&self, track_id: i64) -> Result<Arc<Track>>;
    fn list_albums_search(&self) -> Result<Vec<(u32, String, String)>>;
    fn add_playlist_item(&self, playlist_id: i64, track_id: i64) -> Result<i64>;
    fn create_playlist(&self, name: &str) -> Result<i64>;
    fn delete_playlist(&self, playlist_id: i64) -> Result<()>;
    fn get_all_playlists(&self) -> Result<Arc<Vec<PlaylistWithCount>>>;
    fn get_playlist(&self, playlist_id: i64) -> Result<Arc<Playlist>>;
    fn get_playlist_track_files(&self, playlist_id: i64) -> Result<Arc<Vec<String>>>;
    fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Arc<Vec<(i64, i64, i64)>>>;
    fn move_playlist_item(&self, item_id: i64, new_position: i64) -> Result<()>;
    fn remove_playlist_item(&self, item_id: i64) -> Result<()>;
    fn get_playlist_item(&self, item_id: i64) -> Result<PlaylistItem>;
    fn get_track_stats(&self) -> Result<Arc<TrackStats>>;
    fn playlist_has_track(&self, playlist_id: i64, track_id: i64) -> Result<Option<i64>>;
}

impl LibraryAccess for App {
    fn list_albums(&self, sort_method: AlbumSortMethod) -> Result<Vec<(u32, String)>> {
        let pool: &Pool = self.global();
        block_on(list_albums(&pool.0, sort_method))
    }

    fn list_tracks_in_album(&self, album_id: i64) -> Result<Arc<Vec<Track>>> {
        let pool: &Pool = self.global();
        block_on(list_tracks_in_album(&pool.0, album_id))
    }

    fn get_album_by_id(&self, album_id: i64, method: AlbumMethod) -> Result<Arc<Album>> {
        let pool: &Pool = self.global();
        block_on(get_album_by_id(&pool.0, album_id, method))
    }

    fn get_artist_name_by_id(&self, artist_id: i64) -> Result<Arc<String>> {
        let pool: &Pool = self.global();
        block_on(get_artist_name_by_id(&pool.0, artist_id))
    }

    fn get_artist_by_id(&self, artist_id: i64) -> Result<Arc<Artist>> {
        let pool: &Pool = self.global();
        block_on(get_artist_by_id(&pool.0, artist_id))
    }

    fn get_track_by_id(&self, track_id: i64) -> Result<Arc<Track>> {
        let pool: &Pool = self.global();
        block_on(get_track_by_id(&pool.0, track_id))
    }

    /// Lists all albums for searching. Returns a vector of tuples containing the id, name, and artist
    /// name.
    fn list_albums_search(&self) -> Result<Vec<(u32, String, String)>> {
        let pool: &Pool = self.global();
        block_on(list_albums_search(&pool.0))
    }

    fn add_playlist_item(&self, playlist_id: i64, track_id: i64) -> Result<i64> {
        let pool: &Pool = self.global();
        block_on(add_playlist_item(&pool.0, playlist_id, track_id))
    }

    fn create_playlist(&self, name: &str) -> Result<i64> {
        let pool: &Pool = self.global();
        block_on(create_playlist(&pool.0, name))
    }

    fn delete_playlist(&self, playlist_id: i64) -> Result<()> {
        let pool: &Pool = self.global();
        block_on(delete_playlist(&pool.0, playlist_id))
    }

    fn get_all_playlists(&self) -> Result<Arc<Vec<PlaylistWithCount>>> {
        let pool: &Pool = self.global();
        block_on(get_all_playlists(&pool.0))
    }

    fn get_playlist(&self, playlist_id: i64) -> Result<Arc<Playlist>> {
        let pool: &Pool = self.global();
        block_on(get_playlist(&pool.0, playlist_id))
    }

    fn get_playlist_track_files(&self, playlist_id: i64) -> Result<Arc<Vec<String>>> {
        let pool: &Pool = self.global();
        block_on(get_playlist_track_files(&pool.0, playlist_id))
    }

    fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Arc<Vec<(i64, i64, i64)>>> {
        let pool: &Pool = self.global();
        block_on(get_playlist_tracks(&pool.0, playlist_id))
    }

    fn move_playlist_item(&self, item_id: i64, new_position: i64) -> Result<()> {
        let pool: &Pool = self.global();
        block_on(move_playlist_item(&pool.0, item_id, new_position))
    }

    fn remove_playlist_item(&self, item_id: i64) -> Result<()> {
        let pool: &Pool = self.global();
        block_on(remove_playlist_item(&pool.0, item_id))
    }

    fn get_playlist_item(&self, item_id: i64) -> Result<PlaylistItem> {
        let pool: &Pool = self.global();
        block_on(get_playlist_item(&pool.0, item_id))
    }

    fn get_track_stats(&self) -> Result<Arc<TrackStats>> {
        let pool: &Pool = self.global();
        block_on(get_track_stats(&pool.0))
    }

    fn playlist_has_track(&self, playlist_id: i64, track_id: i64) -> Result<Option<i64>> {
        let pool: &Pool = self.global();
        block_on(playlist_has_track(&pool.0, playlist_id, track_id))
    }
}
