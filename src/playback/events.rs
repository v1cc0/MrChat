#![allow(dead_code)]

use crate::media::metadata::Metadata;

use super::{queue::QueueItemData, thread::PlaybackState};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum RepeatState {
    NotRepeating,
    Repeating,
    RepeatingOne,
}

/// A command to the playback thread. This is used to control the playback thread from other
/// threads. The playback thread recieves these commands from an MPSC channel, and processes them
/// in the order they are recieved. They are processed every 10ms when playback is stopped, or
/// every time additional decoding is required to fill the ring buffer during playback.
#[derive(Debug, PartialEq, Clone)]
pub enum PlaybackCommand {
    /// Requests that the playback thread begin playback.
    Play,
    /// Requests that the playback thread pause playback.
    Pause,
    /// Requests that, if the playback thread is playing, it pauses, and vise/versa.
    TogglePlayPause,
    /// Requests that the playback thread open the specified file for immediate playback.
    Open(PathBuf),
    /// Requests that the playback thread queue the specified file for playback after the current
    /// file. If there is no current file, the specified file will be played immediately.
    Queue(QueueItemData),
    /// Requests that the playback thread queue a list of files for playback after the current
    /// file. If there is no current file, the first file in the list will be played immediately.
    QueueList(Vec<QueueItemData>),
    /// Requests that the playback thread skip to the next file in the queue.
    Next,
    /// Requests that the playback thread skip to the previous file in the queue.
    /// If the current file is more than 5 seconds in, it will be restarted.
    Previous,
    /// Requests that the playback thread clear the queue.
    ClearQueue,
    /// Jumps to the specified position in the queue.
    Jump(usize),
    /// Jumps to the specified position in the queue. This will use the position of the track
    /// in the *unshuffled* queue, regardless of the current shuffle state.
    JumpUnshuffled(usize),
    /// Requests that the playback thread seek to the specified position in the current file.
    Seek(f64),
    /// Requests that the playback thread set the volume to the specified level.
    SetVolume(f64),
    /// Requests that the playback thread replace the current queue with the specified queue.
    /// This will set the current playing track to the first item in the queue.
    ReplaceQueue(Vec<QueueItemData>),
    /// Requests that the playback thread stop playback.
    Stop,
    /// Requests that the playback thread shuffle (or stop shuffling) the next tracks in the
    /// queue. Note that this currently results in duplication of the *entire* queue.
    ToggleShuffle,
    /// Requests that the repeating setting should be set to the specified RepeatState.
    SetRepeat(RepeatState),
}

/// An event from the playback thread. This is used to communicate information from the playback
/// thread to other threads. The playback thread sends these events to an MPSC channel, and the
/// main thread processes them in the order they are recieved.
#[derive(Debug, PartialEq, Clone)]
pub enum PlaybackEvent {
    /// Indicates that the playback state has changed.
    StateChanged(PlaybackState),
    /// Indicates that the current file has changed providing the path to the new file.
    SongChanged(PathBuf),
    /// Indicates that the duration of the current file has changed. The f64 is the new duration,
    /// in seconds.
    DurationChanged(u64),
    /// Indicates that the queue has been updated.
    QueueUpdated,
    /// Indicates that the position in the queue has changed. The usize is the new position.
    QueuePositionChanged(usize),
    /// Indicates that the MediaProvider has provided new metadata to be consumed by the user
    /// interface. The Metadata is boxed to avoid enum size bloat.
    MetadataUpdate(Box<Metadata>),
    /// Indicates that the MediaProvider has provided a new album art image to be consumed by the
    /// user interface.
    AlbumArtUpdate(Option<Box<[u8]>>),
    /// Indicates that the position in the current file has changed. The f64 is the new position,
    /// in seconds.
    PositionChanged(u64),
    /// Notification for when shuffling is disabled or enabled by the thread.
    ShuffleToggled(bool, usize),
    /// Indicates that repeat state has been changed.
    RepeatChanged(RepeatState),
    /// Indicates that the volume has changed. The f64 is the new volume, from 0.0 to 1.0.
    VolumeChanged(f64),
}
