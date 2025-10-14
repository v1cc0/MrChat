// On Windows do NOT show a console window when opening the app
#![cfg_attr(
    all(not(test), not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use player::services::mmb::lastfm::{LASTFM_API_KEY, LASTFM_API_SECRET};
use smol_macros::main;

mod chat;
mod player;
mod shared;

main! {
    async fn main() {
        // Configure tracing with local time instead of UTC
        tracing_subscriber::fmt()
            .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
            .init();

        tracing::info!("Starting application");

        if LASTFM_API_KEY.is_none() || LASTFM_API_SECRET.is_none() {
            tracing::warn!("Binary not compiled with LastFM support, set LASTFM_API_KEY and LASTFM_API_SECRET at compile time to enable");
        }

        crate::player::ui::app::run().await;
    }
}
