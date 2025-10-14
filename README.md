# MrChat

A modern AI chat application with integrated music player, written in Rust using GPUI.

## Features

### AI Chat (In Development)
- Multi-session conversation management
- Support for multiple LLM providers
- Turso database for persistent storage
- Real-time streaming responses

### Music Player (Functional)
- Native audio playback (no web components)
- Format support: FLAC, MP3, OGG (Vorbis), AAC, WAV
- Turso/libSQL-backed music library
- Desktop integration (MPRIS on Linux, MediaPlayer on macOS/Windows)
- Last.fm scrobbling support
- Playlist management
- Fuzzy-find album search
- Theming with hot reload

## Platform Support
- Linux
- macOS
- Windows

## Installation

### From Source

```sh
# Install system dependencies (Linux)
# - xcb-common, x11, wayland development packages
# - openssl development packages
# - pulseaudio development packages (optional)

git clone https://github.com/yourusername/mrchat
cd mrchat

# Optional: Configure Last.fm scrobbling
# Get API keys from https://www.last.fm/api/account/create
export LASTFM_API_KEY="your_key"
export LASTFM_API_SECRET="your_secret"

# Build release version
cargo build --release

# Run
./target/release/mrchat
```

### Configuration

Create `config.toml` in your data directory (`~/.local/share/mrchat/` on Linux):

```toml
[app]
name = "MrChat"

[chat]
default_model = "gpt-4"
api_base_url = "https://api.openai.com/v1"

[player]
scan_directories = ["~/Music"]
always_repeat = false

[turso]
music_db_path = "~/.local/share/mrchat/music.db"
chat_db_path = "~/.local/share/mrchat/mrchat.db"
```

## Development Status

### v0.0.2 (Current)
- ✅ Complete Turso database migration
- ✅ Music playback fully functional
- ✅ Playlist management
- ✅ Desktop integration
- ✅ Self-contained binary with embedded migrations
- 🚧 AI chat interface (basic UI complete)
- 🚧 LLM provider integration (in progress)

### Planned Features
- Advanced chat features (context management, export/import)
- Multiple LLM provider support
- Plugin system for chat and music
- Advanced music library management
- Lyrics support
- ReplayGain support

## Architecture

MrChat is built on a modular architecture with three main components:

- **`src/chat/`** - AI chat functionality
- **`src/player/`** - Music player functionality
- **`src/shared/`** - Shared utilities and UI components

See `docs/chat_player_architecture.md` for detailed design documentation.

## Known Issues

- Symphonia MP3 decoder may output `invalid main_data_begin` warnings for some files (does not affect playback)
- See `docs/turso_database_issues.md` for Turso-specific known issues and workarounds

## Contributing

Contributions are welcome! Please ensure:
- Code is formatted with `rustfmt`
- No new compiler warnings (unused code warnings are acceptable for WIP features)
- Changes work on Linux and macOS (Windows testing is appreciated but not required)

## License

Apache-2.0

## Acknowledgments

MrChat's music player is based on [Hummingbird](https://github.com/143mailliw/hummingbird), a modern music player built with GPUI.
