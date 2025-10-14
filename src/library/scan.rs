use std::{
    fs::{self, File},
    io::{BufReader, Cursor, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use ahash::AHashMap;
use anyhow::Context;
use async_channel::{Receiver, Sender};
use globwalk::GlobWalkerBuilder;
use gpui::{App, Global};
use image::{DynamicImage, EncodableLayout, codecs::jpeg::JpegEncoder, imageops::thumbnail};
use smol::{Timer, block_on};
use tracing::{debug, error, info, warn};

use crate::db::{TursoConnection, TursoDatabase};

use crate::{
    media::{
        builtin::symphonia::SymphoniaProvider,
        metadata::Metadata,
        traits::{MediaPlugin, MediaProvider},
    },
    settings::scan::ScanSettings,
    ui::{app::get_dirs, models::Models},
};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScanEvent {
    Cleaning,
    DiscoverProgress(u64),
    ScanProgress { current: u64, total: u64 },
    ScanCompleteWatching,
    ScanCompleteIdle,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum ScanCommand {
    Scan,
    Stop,
}

pub struct ScanInterface {
    events_rx: Option<Receiver<ScanEvent>>,
    command_tx: Sender<ScanCommand>,
}

impl ScanInterface {
    pub(self) fn new(
        events_rx: Option<Receiver<ScanEvent>>,
        command_tx: Sender<ScanCommand>,
    ) -> Self {
        ScanInterface {
            events_rx,
            command_tx,
        }
    }

    pub fn scan(&self) {
        let command_tx = self.command_tx.clone();
        smol::spawn(async move {
            command_tx
                .send(ScanCommand::Scan)
                .await
                .expect("could not send tx");
        })
        .detach();
    }

    pub fn stop(&self) {
        let command_tx = self.command_tx.clone();
        smol::spawn(async move {
            command_tx
                .send(ScanCommand::Stop)
                .await
                .expect("could not send tx");
        })
        .detach();
    }

    pub fn start_broadcast(&mut self, cx: &mut App) {
        let mut events_rx = None;
        std::mem::swap(&mut self.events_rx, &mut events_rx);

        let state_model = cx.global::<Models>().scan_state.clone();

        let Some(events_rx) = events_rx else {
            return;
        };
        cx.spawn(async move |cx| {
            loop {
                while let Ok(event) = events_rx.recv().await {
                    state_model
                        .update(cx, |m, cx| {
                            *m = event;
                            cx.notify()
                        })
                        .expect("failed to update scan state model");
                }
            }
        })
        .detach();
    }
}

impl Global for ScanInterface {}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScanState {
    Idle,
    Cleanup,
    Discovering,
    Scanning,
}

pub struct ScanThread {
    event_tx: Sender<ScanEvent>,
    command_rx: Receiver<ScanCommand>,
    pool: TursoDatabase,
    scan_settings: ScanSettings,
    visited: Vec<PathBuf>,
    discovered: Vec<PathBuf>,
    to_process: Vec<PathBuf>,
    scan_state: ScanState,
    provider_table: Vec<(&'static [&'static str], Box<dyn MediaProvider>)>,
    scan_record: AHashMap<PathBuf, u64>,
    scan_record_path: Option<PathBuf>,
    scanned: u64,
    discovered_total: u64,
}

struct TrackCleanupContext {
    album_id: Option<i64>,
    disc_key: i64,
    folder: Option<String>,
}

fn build_provider_table() -> Vec<(&'static [&'static str], Box<dyn MediaProvider>)> {
    // TODO: dynamic plugin loading
    vec![(
        SymphoniaProvider::SUPPORTED_EXTENSIONS,
        Box::new(SymphoniaProvider::default()),
    )]
}

fn file_is_scannable_with_provider(path: &Path, exts: &&[&str]) -> bool {
    for extension in exts.iter() {
        if let Some(ext) = path.extension() {
            if ext == *extension {
                return true;
            }
        }
    }

    false
}

type FileInformation = (Metadata, u64, Option<Box<[u8]>>);

fn scan_file_with_provider(
    path: &PathBuf,
    provider: &mut Box<dyn MediaProvider>,
) -> Result<FileInformation, ()> {
    let src = std::fs::File::open(path).map_err(|_| ())?;
    provider.open(src, None).map_err(|_| ())?;
    provider.start_playback().map_err(|_| ())?;
    let metadata = provider.read_metadata().cloned().map_err(|_| ())?;
    let image = provider.read_image().map_err(|_| ())?;
    let len = provider.duration_secs().map_err(|_| ())?;
    provider.close().map_err(|_| ())?;
    Ok((metadata, len, image))
}

// Returns the first image (cover/front/folder.jpeg/png/jpeg) in the track's containing folder
// Album art can be named anything, but this pattern is convention and the least likely to return a false positive
fn scan_path_for_album_art(path: &Path) -> Option<Box<[u8]>> {
    let glob = GlobWalkerBuilder::from_patterns(
        path.parent().unwrap(),
        &["{folder,cover,front}.{jpg,jpeg,png}"],
    )
    .case_insensitive(true)
    .max_depth(1)
    .build()
    .expect("Failed to build album art glob")
    .filter_map(|e| e.ok());

    for entry in glob {
        if let Ok(bytes) = fs::read(entry.path()) {
            return Some(bytes.into_boxed_slice());
        }
    }
    None
}

fn is_db_locked(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("database is locked"))
}

impl ScanThread {
    pub fn start(pool: TursoDatabase, settings: ScanSettings) -> ScanInterface {
        let (commands_tx, commands_rx) = async_channel::bounded(10);
        let (events_tx, events_rx) = async_channel::unbounded();

        std::thread::Builder::new()
            .name("scanner".to_string())
            .spawn(move || {
                let mut thread = ScanThread {
                    event_tx: events_tx,
                    command_rx: commands_rx,
                    pool,
                    visited: Vec::new(),
                    discovered: Vec::new(),
                    to_process: Vec::new(),
                    scan_state: ScanState::Idle,
                    provider_table: build_provider_table(),
                    scan_settings: settings,
                    scan_record: AHashMap::new(),
                    scan_record_path: None,
                    scanned: 0,
                    discovered_total: 0,
                };

                thread.run();
            })
            .expect("could not start playback thread");

        ScanInterface::new(Some(events_rx), commands_tx)
    }

    fn run(&mut self) {
        let dirs = get_dirs();
        let directory = dirs.data_dir();
        if !directory.exists() {
            fs::create_dir(directory).expect("couldn't create data directory");
        }
        let file_path = directory.join("scan_record.json");

        if file_path.exists() {
            let file = File::open(&file_path);

            let Ok(file) = file else {
                return;
            };
            let reader = BufReader::new(file);

            match serde_json::from_reader(reader) {
                Ok(scan_record) => {
                    self.scan_record = scan_record;
                }
                Err(e) => {
                    error!("could not read scan record: {:?}", e);
                    error!("scanning will be slow until the scan record is rebuilt");
                }
            }
        }

        self.scan_record_path = Some(file_path);

        loop {
            self.read_commands();

            // TODO: start file watcher to update db automatically when files are added or removed
            match self.scan_state {
                ScanState::Idle => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                ScanState::Cleanup => {
                    self.cleanup();
                }
                ScanState::Discovering => {
                    self.discover();
                }
                ScanState::Scanning => {
                    self.scan();
                }
            }
        }
    }

    fn read_commands(&mut self) {
        while let Ok(command) = self.command_rx.try_recv() {
            match command {
                ScanCommand::Scan => {
                    if self.scan_state == ScanState::Idle {
                        self.discovered = self.scan_settings.paths.clone();
                        self.scan_state = ScanState::Cleanup;
                        self.scanned = 0;
                        self.discovered_total = 0;

                        let event_tx = self.event_tx.clone();
                        smol::spawn(async move {
                            event_tx
                                .send(ScanEvent::Cleaning)
                                .await
                                .expect("could not send scan started event");
                        })
                        .detach();
                    }
                }
                ScanCommand::Stop => {
                    self.scan_state = ScanState::Idle;
                    self.visited.clear();
                    self.discovered.clear();
                    self.to_process.clear();
                }
            }
        }

        if self.scan_state == ScanState::Discovering {
            self.discover();
        } else if self.scan_state == ScanState::Scanning {
            self.scan();
        } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn file_is_scannable(&mut self, path: &PathBuf) -> bool {
        let timestamp = match fs::metadata(path) {
            Ok(metadata) => metadata
                .modified()
                .unwrap()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Err(_) => return false,
        };

        for (exts, _) in self.provider_table.iter() {
            let x = file_is_scannable_with_provider(path, exts);

            if !x {
                continue;
            }
            if let Some(last_scan) = self.scan_record.get(path) {
                if *last_scan == timestamp {
                    return false;
                }
            }

            self.scan_record.insert(path.clone(), timestamp);
            return true;
        }

        false
    }

    fn discover(&mut self) {
        if self.discovered.is_empty() {
            self.scan_state = ScanState::Scanning;
            return;
        }

        let path = self.discovered.pop().unwrap();

        if self.visited.contains(&path) {
            return;
        }

        let paths = fs::read_dir(&path).unwrap();

        for paths in paths {
            // TODO: handle errors
            // this might be slower than just reading the path directly but this prevents loops
            let path = paths.unwrap().path().canonicalize().unwrap();
            if path.is_dir() {
                self.discovered.push(path);
            } else if self.file_is_scannable(&path) {
                self.to_process.push(path);

                self.discovered_total += 1;

                if self.discovered_total % 20 == 0 {
                    let event_tx = self.event_tx.clone();
                    let discovered_total = self.discovered_total;
                    smol::spawn(async move {
                        event_tx
                            .send(ScanEvent::DiscoverProgress(discovered_total))
                            .await
                            .expect("could not send discovered event");
                    })
                    .detach();
                }
            }
        }

        self.visited.push(path.clone());
    }

    async fn insert_artist(&self, conn: &TursoConnection, metadata: &Metadata) -> anyhow::Result<Option<i64>> {
        let artist = metadata.album_artist.clone().or(metadata.artist.clone());

        let Some(artist) = artist else {
            return Ok(None);
        };

        // Try to insert, returns id if successful, None if conflict
        let result = conn
            .query_optional(
                include_str!("../../queries/scan/create_artist.sql"),
                (
                    artist.as_str(),
                    metadata.artist_sort.as_ref().unwrap_or(&artist).as_str(),
                ),
                |row| Ok(row.get::<i64>(0)?),
            )
            .await?;

        if let Some(id) = result {
            return Ok(Some(id));
        }

        // Artist already exists, fetch the id
        let id = conn
            .query_one(
                include_str!("../../queries/scan/get_artist_id.sql"),
                (artist.as_str(),),
                |row| Ok(row.get::<i64>(0)?),
            )
            .await?;

        Ok(Some(id))
    }

    async fn insert_album(
        &self,
        conn: &TursoConnection,
        metadata: &Metadata,
        artist_id: Option<i64>,
        image: &Option<Box<[u8]>>,
    ) -> anyhow::Result<Option<i64>> {
        let Some(album) = &metadata.album else {
            return Ok(None);
        };

        let mbid = metadata
            .mbid_album
            .clone()
            .unwrap_or_else(|| "none".to_string());

        // Check if album already exists
        let existing = conn
            .query_optional(
                include_str!("../../queries/scan/get_album_id.sql"),
                (album.as_str(), mbid.as_str()),
                |row| Ok(row.get::<i64>(0)?),
            )
            .await?;

        if let Some(id) = existing {
            return Ok(Some(id));
        }

        // Album doesn't exist, create it
        let (resized_image, thumb) = match image {
            Some(image) => {
                // if there is a decode error, just ignore it and pretend there is no image
                let mut decoded = image::ImageReader::new(Cursor::new(&image))
                    .with_guessed_format()?
                    .decode()?
                    .into_rgb8();

                // for some reason, thumbnails don't load properly when saved as rgb8
                // also, into_rgba8() causes the application to crash on certain images
                //
                // no, I don't no why, and no I can't fix it upstream
                // this will have to do for now
                let decoded_rgba = DynamicImage::ImageRgb8(decoded.clone()).into_rgba8();

                let thumb = thumbnail(&decoded_rgba, 70, 70);

                let mut buf: Cursor<Vec<u8>> = Cursor::new(Vec::new());

                thumb
                    .write_to(&mut buf, image::ImageFormat::Bmp)
                    .expect("i don't know how Cursor could fail");
                buf.flush().expect("could not flush buffer");

                let resized = if decoded.dimensions().0 <= 1024 || decoded.dimensions().1 <= 1024 {
                    image.clone().to_vec()
                } else {
                    decoded = image::imageops::resize(
                        &decoded,
                        1024,
                        1024,
                        image::imageops::FilterType::Lanczos3,
                    );
                    let mut buf: Cursor<Vec<u8>> = Cursor::new(Vec::new());
                    let mut encoder = JpegEncoder::new_with_quality(&mut buf, 70);

                    encoder.encode(
                        decoded.as_bytes(),
                        decoded.width(),
                        decoded.height(),
                        image::ExtendedColorType::Rgb8,
                    )?;
                    buf.flush()?;

                    buf.get_mut().clone()
                };

                (Some(resized), Some(buf.get_mut().clone()))
            }
            None => (None, None),
        };

        let id = conn
            .query_one(
                include_str!("../../queries/scan/create_album.sql"),
                (
                    album.as_str(),
                    metadata.sort_album.as_ref().unwrap_or(album).as_str(),
                    artist_id,
                    resized_image,
                    thumb,
                    metadata.date.map(|d| d.timestamp()),
                    metadata.label.as_deref(),
                    metadata.catalog.as_deref(),
                    metadata.isrc.as_deref(),
                    mbid.as_str(),
                ),
                |row| Ok(row.get::<i64>(0)?),
            )
            .await
            .with_context(|| {
                format!(
                    "failed to insert album: title={:?} artist_id={:?} mbid={:?}",
                    album, artist_id, mbid
                )
            })?;

        Ok(Some(id))
    }

    async fn insert_track(
        &self,
        conn: &TursoConnection,
        metadata: &Metadata,
        album_id: Option<i64>,
        path: &Path,
        length: u64,
    ) -> anyhow::Result<()> {
        if album_id.is_none() {
            return Ok(());
        }

        let disc_num = metadata.disc_current.map(|v| v as i64).unwrap_or(-1);
        let parent = path.parent().unwrap();

        let existing_path = conn
            .query_optional(
                include_str!("../../queries/scan/get_album_path.sql"),
                (album_id, disc_num),
                |row| Ok(row.get::<String>(0)?),
            )
            .await?;

        match existing_path {
            Some(path) => {
                if path.as_str() != parent.as_os_str() {
                    return Ok(());
                }
            }
            None => {
                conn.execute(
                    include_str!("../../queries/scan/create_album_path.sql"),
                    (album_id, parent.to_str(), disc_num),
                )
                .await?;
            }
        }

        let name = metadata
            .name
            .clone()
            .or_else(|| {
                path.file_name()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_string())
            })
            .ok_or_else(|| anyhow::anyhow!("failed to retrieve filename"))?;

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("path contains invalid UTF-8: {:?}", path))?;

        let parent_str = parent
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("parent path contains invalid UTF-8: {:?}", parent))?;

        conn.query_one(
            include_str!("../../queries/scan/create_track.sql"),
            (
                name.as_str(),
                name.as_str(),
                album_id,
                metadata.track_current.map(|x| x as i32),
                metadata.disc_current.map(|x| x as i32),
                length as i32,
                path_str,
                metadata.genre.as_deref(),
                metadata.artist.as_deref(),
                parent_str,
            ),
            |row| Ok(row.get::<i64>(0)?),
        )
        .await?;

        Ok(())
    }

    async fn update_metadata_once(
        &mut self,
        metadata: &FileInformation,
        path: &Path,
    ) -> anyhow::Result<()> {
        let (meta, length, image) = metadata;

        debug!(
            "Adding/updating record for {:?} - {:?}",
            meta.artist, meta.name
        );

        // Use a single connection for the entire metadata update
        let conn = self.pool.connect()?;

        let artist_id = self.insert_artist(&conn, meta).await?;
        let album_id = self.insert_album(&conn, meta, artist_id, image).await?;
        self.insert_track(&conn, meta, album_id, path, *length).await?;

        Ok(())
    }

    async fn update_metadata(
        &mut self,
        metadata: FileInformation,
        path: &Path,
    ) -> anyhow::Result<()> {
        const MAX_RETRY: usize = 5;
        for attempt in 0..=MAX_RETRY {
            match self.update_metadata_once(&metadata, path).await {
                Ok(()) => return Ok(()),
                Err(err) if is_db_locked(&err) && attempt < MAX_RETRY => {
                    let delay_ms = 50 * (attempt as u64 + 1);
                    Timer::after(std::time::Duration::from_millis(delay_ms)).await;
                }
                Err(err) => return Err(err),
            }
        }

        // Should be unreachable because loop returns on success or final error
        Err(anyhow::anyhow!("database locked after retries"))
    }

    fn read_metadata_for_path(&mut self, path: &PathBuf) -> Option<FileInformation> {
        for (exts, provider) in &mut self.provider_table {
            if file_is_scannable_with_provider(path, exts) {
                if let Ok(mut metadata) = scan_file_with_provider(path, provider) {
                    if metadata.2.is_none() {
                        metadata.2 = scan_path_for_album_art(path);
                    }

                    return Some(metadata);
                }
            }
        }

        None
    }

    fn write_scan_record(&self) {
        if let Some(path) = self.scan_record_path.as_ref() {
            let mut file = File::create(path).unwrap();
            let data = serde_json::to_string(&self.scan_record).unwrap();
            if let Err(err) = file.write_all(data.as_bytes()) {
                error!("Could not write scan record: {:?}", err);
                error!("Scan record will not be saved, this may cause rescans on restart");
            } else {
                info!("Scan record written to {:?}", path);
            }
        } else {
            error!("No scan record path set, scan record will not be saved");
        }
    }

    fn scan(&mut self) {
        if self.to_process.is_empty() {
            info!("Scan complete, writing scan record and stopping");
            self.write_scan_record();
            self.scan_state = ScanState::Idle;
            let event_tx = self.event_tx.clone();
            smol::spawn(async move {
                event_tx.send(ScanEvent::ScanCompleteIdle).await.unwrap();
            })
            .detach();
            return;
        }

        let path = self.to_process.pop().unwrap();
        let metadata = self.read_metadata_for_path(&path);

        if let Some(metadata) = metadata {
            let result = block_on(self.update_metadata(metadata, &path));

            if let Err(err) = result {
                error!(
                    "Failed to update metadata for file: {:?}, error: {err:#?}",
                    path
                );
            }

            self.scanned += 1;

            if self.scanned % 5 == 0 {
                let event_tx = self.event_tx.clone();
                let scan_progress = ScanEvent::ScanProgress {
                    current: self.scanned,
                    total: self.discovered_total,
                };
                smol::spawn(async move {
                    event_tx.send(scan_progress).await.unwrap();
                })
                .detach();
            }
        } else {
            warn!("Could not read metadata for file: {:?}", path);
        }
    }

    async fn delete_track(&mut self, path: &PathBuf) {
        debug!("track deleted or moved: {:?}", path);
        let conn = match self.pool.connect() {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to connect to database: {:?}", e);
                return;
            }
        };

        let Some(path_str) = path.to_str() else {
            error!(
                "Failed to delete track: path is not valid UTF-8: {:?}",
                path
            );
            return;
        };

        let track_context = match conn
            .query_optional(
                include_str!("../../queries/scan/get_track_cleanup_context.sql"),
                (path_str,),
                |row| {
                    Ok(TrackCleanupContext {
                        album_id: row.get::<Option<i64>>(0)?,
                        disc_key: row.get::<i64>(1)?,
                        folder: row.get::<Option<String>>(2)?,
                    })
                },
            )
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => {
                error!(
                    "Database error while fetching track cleanup context for {:?}: {:?}",
                    path, e
                );
                None
            }
        };

        let result = conn
            .execute(
                include_str!("../../queries/scan/delete_track.sql"),
                (path_str,),
            )
            .await;

        if let Err(e) = result {
            error!("Database error while deleting track: {:?}", e);
        } else {
            self.scan_record.remove(path);
            if let Some(ctx) = track_context {
                if let Err(e) = self.cleanup_track_removal(&conn, &ctx).await {
                    error!(
                        "Database error while cleaning up after track deletion {:?}: {:?}",
                        path, e
                    );
                }
            }
        }
    }

    async fn cleanup_track_removal(
        &self,
        conn: &TursoConnection,
        ctx: &TrackCleanupContext,
    ) -> anyhow::Result<()> {
        let Some(album_id) = ctx.album_id else {
            return Ok(());
        };

        if let Some(folder) = ctx.folder.as_deref() {
            let remaining_in_folder: i64 = conn
                .query_scalar(
                    "SELECT COUNT(1) FROM track WHERE folder = $1 AND IFNULL(disc_number, -1) = $2 AND album_id = $3",
                    (folder, ctx.disc_key, album_id),
                )
                .await?;

            if remaining_in_folder == 0 {
                conn.execute(
                    "DELETE FROM album_path WHERE album_id = $1 AND path = $2 AND disc_num = $3",
                    (album_id, folder, ctx.disc_key),
                )
                .await?;
            }
        }

        let tracks_remaining: i64 = conn
            .query_scalar(
                "SELECT COUNT(1) FROM track WHERE album_id = $1",
                (album_id,),
            )
            .await?;

        if tracks_remaining > 0 {
            return Ok(());
        }

        let artist_id: Option<i64> = conn
            .query_scalar_optional("SELECT artist_id FROM album WHERE id = $1", (album_id,))
            .await?;

        conn.execute("DELETE FROM album_path WHERE album_id = $1", (album_id,))
            .await?;

        conn.execute(
            include_str!("../../queries/scan/delete_album.sql"),
            (album_id,),
        )
        .await?;

        if let Some(artist_id) = artist_id {
            let albums_remaining: i64 = conn
                .query_scalar(
                    "SELECT COUNT(1) FROM album WHERE artist_id = $1",
                    (artist_id,),
                )
                .await?;

            if albums_remaining == 0 {
                conn.execute(
                    include_str!("../../queries/scan/delete_artist.sql"),
                    (artist_id,),
                )
                .await?;
            }
        }

        Ok(())
    }

    // This is done in one shot because it's required for data integrity
    // Cleanup cannot be cancelled
    fn cleanup(&mut self) {
        self.scan_record
            .clone()
            .iter()
            .filter(|v| !v.0.exists())
            .map(|v| v.0)
            .for_each(|v| {
                block_on(self.delete_track(v));
            });

        self.scan_state = ScanState::Discovering;
    }
}
