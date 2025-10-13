use serde::{Deserialize, Serialize};

use crate::ui::models::CurrentTrack;

use std::{fs, path::PathBuf};

/// Data to store while quitting the app
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageData {
    pub current_track: Option<CurrentTrack>,
}

#[derive(Debug, Clone)]
pub struct Storage {
    /// File path to store data
    path: PathBuf,
}

impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Save `StorageData` on file system
    pub fn save(&self, data: &StorageData) {
        // save into file
        let result = fs::File::create(self.path.clone())
            .and_then(|file| serde_json::to_writer(file, &data).map_err(|e| e.into()));
        // ignore error, but log it
        if let Err(e) = result {
            tracing::warn!("could not save `AppState` {:?}", e);
        };
    }

    /// Load `StorageData` from storage or use `StorageData::default` in case of any errors
    pub fn load_or_default(&self) -> StorageData {
        std::fs::File::open(self.path.clone())
            .and_then(|file| {
                serde_json::from_reader(file)
                    .map_err(|e| e.into())
                    .map(|data: StorageData| match &data.current_track {
                        // validate whether path still exists
                        Some(current_track) if !current_track.get_path().exists() => StorageData {
                            current_track: None,
                        },
                        _ => data,
                    })
            })
            .unwrap_or_default()
    }
}
