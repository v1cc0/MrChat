#![allow(dead_code)]
pub mod table;

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use gpui::{IntoElement, RenderImage, SharedString};
use image::{Frame, RgbaImage};
use smallvec::SmallVec;

use crate::util::rgb_to_bgr;

pub struct Artist {
    pub id: i64,
    pub name: Option<DBString>,
    pub name_sortable: Option<String>,
    pub bio: Option<DBString>,
    pub created_at: DateTime<Utc>,
    pub image: Option<Box<[u8]>>,
    pub image_mime: Option<DBString>,
    pub tags: Option<Vec<String>>,
}

impl Artist {
    pub fn from_row(row: &turso::Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0).context("failed to get id")?,
            name: row.get::<Option<String>>(1).context("failed to get name")?.map(DBString::from),
            name_sortable: row.get(2).context("failed to get name_sortable")?,
            bio: row.get::<Option<String>>(3).context("failed to get bio")?.map(DBString::from),
            created_at: row.get::<String>(4).context("failed to get created_at")?
                .parse().context("failed to parse created_at")?,
            image: row.get::<Option<Vec<u8>>>(5).context("failed to get image")?
                .map(|v| v.into_boxed_slice()),
            image_mime: row.get::<Option<String>>(6).context("failed to get image_mime")?.map(DBString::from),
            tags: None,
        })
    }
}

#[derive(Clone)]
pub struct Thumbnail(pub Arc<RenderImage>);

impl Thumbnail {
    pub fn new(image: Arc<RenderImage>) -> Self {
        Self(image)
    }
}

impl From<Box<[u8]>> for Thumbnail {
    fn from(data: Box<[u8]>) -> Self {
        let mut image = image::load_from_memory(&data)
            .unwrap()
            .as_rgba8()
            .map(|image| image.to_owned())
            .unwrap_or_else(|| {
                let mut image = RgbaImage::new(1, 1);
                image.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
                image
            });

        rgb_to_bgr(&mut image);

        Self(Arc::new(RenderImage::new(SmallVec::from_vec(vec![
            Frame::new(image),
        ]))))
    }
}

impl From<Vec<u8>> for Thumbnail {
    fn from(data: Vec<u8>) -> Self {
        Self::from(data.into_boxed_slice())
    }
}

#[derive(Clone, Default, Debug)]
pub struct DBString(pub SharedString);

impl From<String> for DBString {
    fn from(data: String) -> Self {
        Self(SharedString::from(data))
    }
}

impl From<&str> for DBString {
    fn from(data: &str) -> Self {
        Self(SharedString::from(data.to_string()))
    }
}

impl From<DBString> for SharedString {
    fn from(data: DBString) -> Self {
        data.0
    }
}

impl From<DBString> for String {
    fn from(data: DBString) -> Self {
        data.0.to_string()
    }
}

impl std::fmt::Display for DBString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq for DBString {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<String> for DBString {
    fn eq(&self, other: &String) -> bool {
        self.0.as_ref() == other
    }
}

impl PartialEq<DBString> for String {
    fn eq(&self, other: &DBString) -> bool {
        self == other.0.as_ref()
    }
}

impl PartialEq<&str> for DBString {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_ref() == *other
    }
}

impl PartialEq<DBString> for &str {
    fn eq(&self, other: &DBString) -> bool {
        *self == other.0.as_ref()
    }
}

impl IntoElement for DBString {
    type Element = <SharedString as IntoElement>::Element;

    fn into_element(self) -> Self::Element {
        self.0.into_element()
    }
}

#[derive(Clone)]
pub struct Album {
    pub id: i64,
    pub title: DBString,
    pub title_sortable: DBString,
    pub artist_id: i64,
    pub release_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub image: Option<Box<[u8]>>,
    pub thumb: Option<Thumbnail>,
    pub image_mime: Option<String>,
    pub tags: Option<Vec<String>>,
    pub label: Option<DBString>,
    pub catalog_number: Option<DBString>,
    pub isrc: Option<DBString>,
}

impl Album {
    pub fn from_row(row: &turso::Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0).context("failed to get id")?,
            title: DBString::from(row.get::<String>(1).context("failed to get title")?),
            title_sortable: DBString::from(row.get::<String>(2).context("failed to get title_sortable")?),
            artist_id: row.get(3).context("failed to get artist_id")?,
            release_date: row.get::<Option<String>>(4).context("failed to get release_date")?
                .and_then(|s| s.parse().ok()),
            created_at: row.get::<String>(5).context("failed to get created_at")?
                .parse().context("failed to parse created_at")?,
            image: row.get::<Option<Vec<u8>>>(6).context("failed to get image")?
                .map(|v| v.into_boxed_slice()),
            thumb: row.get::<Option<Vec<u8>>>(7).context("failed to get thumb")?.map(Thumbnail::from),
            image_mime: row.get(8).context("failed to get image_mime")?,
            tags: None,
            label: row.get::<Option<String>>(9).context("failed to get label")?.map(DBString::from),
            catalog_number: row.get::<Option<String>>(10).context("failed to get catalog_number")?.map(DBString::from),
            isrc: row.get::<Option<String>>(11).context("failed to get isrc")?.map(DBString::from),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    pub id: i64,
    pub title: DBString,
    pub title_sortable: DBString,
    pub album_id: Option<i64>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration: i64,
    pub created_at: DateTime<Utc>,
    pub genres: Option<Vec<DBString>>,
    pub tags: Option<Vec<DBString>>,
    pub location: PathBuf,
    pub artist_names: Option<DBString>,
}

impl Track {
    pub fn from_row(row: &turso::Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0).context("failed to get id")?,
            title: DBString::from(row.get::<String>(1).context("failed to get title")?),
            title_sortable: DBString::from(row.get::<String>(2).context("failed to get title_sortable")?),
            album_id: row.get(3).context("failed to get album_id")?,
            track_number: row.get(4).context("failed to get track_number")?,
            disc_number: row.get(5).context("failed to get disc_number")?,
            duration: row.get(6).context("failed to get duration")?,
            created_at: row.get::<String>(7).context("failed to get created_at")?
                .parse().context("failed to parse created_at")?,
            genres: None,
            tags: None,
            location: PathBuf::from(row.get::<String>(8).context("failed to get location")?),
            artist_names: row.get::<Option<String>>(9).context("failed to get artist_names")?.map(DBString::from),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(i32)]
pub enum PlaylistType {
    User = 0,
    System = 1,
}

impl PlaylistType {
    pub fn from_i32(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Self::User),
            1 => Ok(Self::System),
            _ => Err(anyhow::anyhow!("invalid playlist type: {}", value)),
        }
    }
}

#[derive(Clone)]
pub struct Playlist {
    pub id: i64,
    pub name: DBString,
    pub created_at: DateTime<Utc>,
    pub playlist_type: PlaylistType,
}

impl Playlist {
    pub fn from_row(row: &turso::Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0).context("failed to get id")?,
            name: DBString::from(row.get::<String>(1).context("failed to get name")?),
            created_at: row.get::<String>(2).context("failed to get created_at")?
                .parse().context("failed to parse created_at")?,
            playlist_type: PlaylistType::from_i32(row.get(3).context("failed to get type")?)?,
        })
    }
}

#[derive(Clone)]
pub struct PlaylistWithCount {
    pub id: i64,
    pub name: DBString,
    pub created_at: DateTime<Utc>,
    pub playlist_type: PlaylistType,
    pub track_count: i64,
}

impl PlaylistWithCount {
    pub fn from_row(row: &turso::Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0).context("failed to get id")?,
            name: DBString::from(row.get::<String>(1).context("failed to get name")?),
            created_at: row.get::<String>(2).context("failed to get created_at")?
                .parse().context("failed to parse created_at")?,
            playlist_type: PlaylistType::from_i32(row.get(3).context("failed to get type")?)?,
            track_count: row.get(4).context("failed to get track_count")?,
        })
    }
}

#[derive(Clone)]
pub struct PlaylistItem {
    pub id: i64,
    pub playlist_id: i64,
    pub track_id: i64,
    pub created_at: DateTime<Utc>,
    pub position: i64,
}

impl PlaylistItem {
    pub fn from_row(row: &turso::Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0).context("failed to get id")?,
            playlist_id: row.get(1).context("failed to get playlist_id")?,
            track_id: row.get(2).context("failed to get track_id")?,
            created_at: row.get::<String>(3).context("failed to get created_at")?
                .parse().context("failed to parse created_at")?,
            position: row.get(4).context("failed to get position")?,
        })
    }
}

#[derive(Clone)]
pub struct TrackStats {
    pub track_count: i64,
    pub total_duration: i64,
}

impl TrackStats {
    pub fn from_row(row: &turso::Row) -> Result<Self> {
        Ok(Self {
            track_count: row.get(0).context("failed to get track_count")?,
            total_duration: row.get(1).context("failed to get total_duration")?,
        })
    }
}
