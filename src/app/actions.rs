//! Application actions and message passing

use crate::subsonic::models::{Album, Artist, Child, Playlist};

/// Actions that can be sent to the audio backend
#[derive(Debug, Clone)]
pub enum AudioAction {
    /// Play a specific song by URL
    Play { url: String, song: Child },
    /// Pause playback
    Pause,
    /// Resume playback
    Resume,
    /// Toggle pause state
    TogglePause,
    /// Stop playback
    Stop,
    /// Seek to position (seconds)
    Seek(f64),
    /// Seek relative to current position
    SeekRelative(f64),
    /// Skip to next track
    Next,
    /// Skip to previous track
    Previous,
    /// Set volume (0-100)
    SetVolume(i32),
}

/// Actions that can be sent to update the UI
#[derive(Debug, Clone)]
pub enum UiAction {
    /// Update playback position
    UpdatePosition { position: f64, duration: f64 },
    /// Update playback state
    UpdatePlaybackState(PlaybackStateUpdate),
    /// Update audio properties
    UpdateAudioProperties {
        sample_rate: Option<u32>,
        bit_depth: Option<u32>,
        format: Option<String>,
    },
    /// Track ended (EOF from MPV)
    TrackEnded,
    /// Show notification
    Notify { message: String, is_error: bool },
    /// Artists loaded from server
    ArtistsLoaded(Vec<Artist>),
    /// Albums loaded for an artist
    AlbumsLoaded {
        artist_id: String,
        albums: Vec<Album>,
    },
    /// Songs loaded for an album
    SongsLoaded { album_id: String, songs: Vec<Child> },
    /// Playlists loaded from server
    PlaylistsLoaded(Vec<Playlist>),
    /// Playlist songs loaded
    PlaylistSongsLoaded {
        playlist_id: String,
        songs: Vec<Child>,
    },
    /// Server connection test result
    ConnectionTestResult { success: bool, message: String },
    /// Force redraw
    Redraw,
}

/// Playback state update
#[derive(Debug, Clone, Copy)]
pub enum PlaybackStateUpdate {
    Playing,
    Paused,
    Stopped,
}

/// Actions for the Subsonic client
#[derive(Debug, Clone)]
pub enum SubsonicAction {
    /// Fetch all artists
    FetchArtists,
    /// Fetch albums for an artist
    FetchAlbums { artist_id: String },
    /// Fetch songs for an album
    FetchAlbum { album_id: String },
    /// Fetch all playlists
    FetchPlaylists,
    /// Fetch songs in a playlist
    FetchPlaylist { playlist_id: String },
    /// Test server connection
    TestConnection,
}

/// Queue manipulation actions
#[derive(Debug, Clone)]
pub enum QueueAction {
    /// Append songs to queue
    Append(Vec<Child>),
    /// Insert songs after current position
    InsertNext(Vec<Child>),
    /// Clear the queue
    Clear,
    /// Remove song at index
    Remove(usize),
    /// Move song from one index to another
    Move { from: usize, to: usize },
    /// Shuffle the queue (keeping current song in place)
    Shuffle,
    /// Play song at queue index
    PlayIndex(usize),
}
