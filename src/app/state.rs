//! Shared application state

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use ratatui::layout::Rect;

use crate::app::models::{BrowseTab, SongOption};
use crate::config::Config;
use crate::subsonic::models::{Album, Artist, Child, InternetRadioStation, Playlist};
use crate::ui::theme::{ThemeColors, ThemeData};

/// Current page in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Browse,
    Artists,
    Queue,
    Playlists,
    Radio,
    Lyrics,
    Server,
    Settings,
}

impl Page {
    pub const DEFAULT_TABS: [Page; 8] = [
        Page::Browse,
        Page::Artists,
        Page::Queue,
        Page::Playlists,
        Page::Radio,
        Page::Lyrics,
        Page::Server,
        Page::Settings,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Page::Browse => "Browse",
            Page::Artists => "Artists",
            Page::Queue => "Queue",
            Page::Playlists => "Playlists",
            Page::Radio => "Radio",
            Page::Lyrics => "Lyrics",
            Page::Server => "Server",
            Page::Settings => "Settings",
        }
    }

    pub fn from_config_name(name: &str) -> Option<Self> {
        match name
            .to_ascii_lowercase()
            .replace([' ', '-', '_'], "")
            .as_str()
        {
            "browse" => Some(Page::Browse),
            "artists" => Some(Page::Artists),
            "queue" => Some(Page::Queue),
            "playlists" => Some(Page::Playlists),
            "radio" => Some(Page::Radio),
            "lyrics" => Some(Page::Lyrics),
            "server" => Some(Page::Server),
            "settings" => Some(Page::Settings),
            _ => None,
        }
    }

    pub fn visible_from_config(names: &[String]) -> Vec<Self> {
        let mut pages = Vec::new();
        for name in names {
            if let Some(page) = Page::from_config_name(name) {
                if !pages.contains(&page) {
                    pages.push(page);
                }
            }
        }
        if pages.is_empty() {
            Page::DEFAULT_TABS.to_vec()
        } else {
            pages
        }
    }
}

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Now playing information
#[derive(Debug, Clone, Default)]
pub struct NowPlaying {
    /// Currently playing song
    pub song: Option<Child>,
    /// Currently playing internet radio station
    pub radio_station: Option<InternetRadioStation>,
    /// Current radio stream title from player metadata
    pub radio_title: Option<String>,
    /// Current radio stream artist from player metadata
    pub radio_artist: Option<String>,
    /// Playback state
    pub state: PlaybackState,
    /// Current position in seconds
    pub position: f64,
    /// Total duration in seconds
    pub duration: f64,
    /// Audio sample rate (Hz)
    pub sample_rate: Option<u32>,
    /// Audio bit depth
    pub bit_depth: Option<u32>,
    /// Audio format/codec
    pub format: Option<String>,
    /// Audio channel layout (e.g., "Stereo", "Mono", "5.1ch")
    pub channels: Option<String>,
    /// Whether the current track has already been scrobbled
    pub scrobbled: bool,
    /// Lyrics for the currently playing song
    pub lyrics: Option<String>,
    /// Parsed lyrics for the currently playing song
    pub parsed_lyrics: Option<crate::app::models::ParsedLyrics>,
    /// Whether we have already checked for lyrics (to avoid redundant polls)
    pub lyrics_checked: bool,
}

impl NowPlaying {
    pub fn progress_percent(&self) -> f64 {
        if self.duration > 0.0 {
            (self.position / self.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn format_position(&self) -> String {
        format_duration(self.position)
    }

    pub fn format_duration(&self) -> String {
        format_duration(self.duration)
    }
}

/// Format duration in MM:SS or HH:MM:SS format
pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}

#[derive(Debug, Clone, Default)]
pub struct BrowseState {
    /// Top-level tab: Songs or Albums
    pub browse_tab: BrowseTab,

    // Songs tab
    /// Currently displayed songs (filtered subset for Starred/Random, paginated for All)
    pub songs: Vec<Child>,
    /// Unfiltered backing store for Starred/Random options
    pub backing_songs: Vec<Child>,
    pub selected_index: Option<usize>,
    pub scroll_offset: usize,
    /// Starred songs list needs refresh before next display
    pub starred_songs_dirty: bool,
    /// Starred albums list needs refresh before next display
    pub starred_albums_dirty: bool,
    /// Current pagination offset for the All option
    pub all_songs_offset: usize,
    /// Whether more songs are available for the All option
    pub all_songs_has_more: bool,
    /// Whether a page of All songs is currently being fetched
    pub all_songs_loading: bool,

    // Albums tab
    /// Currently displayed albums (filtered from backing_albums)
    pub albums: Vec<Album>,
    /// Unfiltered album list for client-side filtering
    pub backing_albums: Vec<Album>,
    /// Selected album index
    pub selected_album: Option<usize>,
    /// Scroll offset for the album list
    pub album_scroll_offset: usize,
    /// Pagination offset for album list fetches
    pub albums_offset: usize,
    /// Whether more albums are available to page in
    pub albums_has_more: bool,
    /// Whether an album page is currently being fetched
    pub albums_loading: bool,

    // Shared
    /// Focus pane:
    ///   0 = Song/Album options pane
    ///   1 = Songs/Albums content list
    /// The browse tab selector is not focusable.
    pub focus: usize,
    /// Current filter text
    pub filter: String,
    /// Whether filter input is active
    pub filter_active: bool,
    pub selected_option: Option<SongOption>,
}

impl BrowseState {
    /// Filter `backing_albums` into `albums` using the current `filter` text.
    /// Resets `selected_album` and `album_scroll_offset` after filtering.
    pub fn apply_album_filter(&mut self) {
        if self.filter.is_empty() {
            self.albums = self.backing_albums.clone();
        } else {
            let lower = self.filter.to_lowercase();
            self.albums = self
                .backing_albums
                .iter()
                .filter(|a| {
                    a.name.to_lowercase().contains(&lower)
                        || a.artist
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&lower)
                })
                .cloned()
                .collect();
        }
        self.selected_album = if self.albums.is_empty() {
            None
        } else {
            Some(0)
        };
        self.album_scroll_offset = 0;
    }

    /// Filter `backing_songs` into `songs` using the current `filter` text.
    /// Resets `selected_index` and `scroll_offset` after filtering.
    pub fn apply_filter(&mut self) {
        if self.filter.is_empty() {
            self.songs = self.backing_songs.clone();
        } else {
            let lower = self.filter.to_lowercase();
            self.songs = self
                .backing_songs
                .iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&lower)
                        || s.artist
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&lower)
                        || s.album
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&lower)
                })
                .cloned()
                .collect();
        }
        self.selected_index = if self.songs.is_empty() { None } else { Some(0) };
        self.scroll_offset = 0;
    }
}

/// Artists page state
#[derive(Debug, Clone, Default)]
pub struct ArtistsState {
    /// List of all artists
    pub artists: Vec<Artist>,
    /// Currently selected index in the tree (artists + expanded albums)
    pub selected_index: Option<usize>,
    /// Set of expanded artist IDs
    pub expanded: std::collections::HashSet<String>,
    /// Albums cached per artist ID
    pub albums_cache: std::collections::HashMap<String, Vec<Album>>,
    /// Songs in the selected album (shown in right pane)
    pub songs: Vec<Child>,
    /// Currently selected song index
    pub selected_song: Option<usize>,
    /// Artist filter text
    pub filter: String,
    /// Whether filter input is active
    pub filter_active: bool,
    /// Focus: 0 = tree, 1 = songs
    pub focus: usize,
    /// Scroll offset for the tree list (set after render)
    pub tree_scroll_offset: usize,
    /// Scroll offset for the songs list (set after render)
    pub song_scroll_offset: usize,
}

impl ArtistsState {
    /// Maximum number of artist album caches to retain.
    pub const MAX_ALBUMS_CACHE: usize = 100;
}

/// Queue page state
#[derive(Debug, Clone, Default)]
pub struct QueueState {
    /// Currently selected index in the queue
    pub selected: Option<usize>,
    /// Scroll offset for the queue list (set after render)
    pub scroll_offset: usize,
}

/// Playlists page state
#[derive(Debug, Clone, Default)]
pub struct PlaylistsState {
    /// List of all playlists
    pub playlists: Vec<Playlist>,
    /// Currently selected playlist index
    pub selected_playlist: Option<usize>,
    /// Songs in the selected playlist
    pub songs: Vec<Child>,
    /// Currently selected song index
    pub selected_song: Option<usize>,
    /// Focus: 0 = playlists, 1 = songs
    pub focus: usize,
    /// Scroll offset for the playlists list (set after render)
    pub playlist_scroll_offset: usize,
    /// Scroll offset for the songs list (set after render)
    pub song_scroll_offset: usize,
}

/// Radio page state
#[derive(Debug, Clone, Default)]
pub struct RadioState {
    /// List of all internet radio stations
    pub stations: Vec<InternetRadioStation>,
    /// Currently selected station index
    pub selected: Option<usize>,
    /// Scroll offset for the station list (set after render)
    pub scroll_offset: usize,
}

/// Lyrics page state
#[derive(Debug, Clone, Default)]
pub struct LyricsState {
    /// Scroll offset for the lyrics text
    pub scroll_offset: usize,
    /// Whether the user is manually scrolling
    pub is_manual_scroll: bool,
    /// Last time the user scrolled
    pub last_scroll_time: Option<Instant>,
    /// Visual line spacing (extra empty lines)
    pub line_spacing: u8,
}

/// Server page state (connection settings)
#[derive(Debug, Clone, Default)]
pub struct ServerState {
    /// Currently focused field (0-4: URL, Username, Password, Test, Save)
    pub selected_field: usize,
    /// Edit values
    pub base_url: String,
    pub username: String,
    pub password: String,
    /// Status message
    pub status: Option<String>,
}

/// Settings page state
#[derive(Debug, Clone)]
pub struct SettingsState {
    /// Currently focused field (0=Theme, 1=Cava)
    pub selected_field: usize,
    /// Available themes (Default + loaded from files)
    pub themes: Vec<ThemeData>,
    /// Index of the currently selected theme in `themes`
    pub theme_index: usize,
    /// Cava visualizer enabled
    pub cava_enabled: bool,
    /// Cava visualizer height percentage (10-80, step 5)
    pub cava_size: u8,
    /// notifications enabled
    pub notifications_enabled: bool,
    /// Scrobbling enabled
    pub scrobble_enabled: bool,
    /// Save and restore queue on launch
    pub save_queue_enabled: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            selected_field: 0,
            themes: vec![ThemeData::default_theme()],
            theme_index: 0,
            cava_enabled: false,
            cava_size: 40,
            notifications_enabled: false,
            scrobble_enabled: true,
            save_queue_enabled: true,
        }
    }
}

impl SettingsState {
    /// Current theme name
    pub fn theme_name(&self) -> &str {
        &self.themes[self.theme_index].name
    }

    /// Current theme colors
    pub fn theme_colors(&self) -> &ThemeColors {
        &self.themes[self.theme_index].colors
    }

    /// Current theme data
    pub fn current_theme(&self) -> &ThemeData {
        &self.themes[self.theme_index]
    }

    /// Cycle to next theme
    pub fn next_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % self.themes.len();
    }

    /// Cycle to previous theme
    pub fn prev_theme(&mut self) {
        self.theme_index = (self.theme_index + self.themes.len() - 1) % self.themes.len();
    }

    /// Set theme by name, returning true if found
    pub fn set_theme_by_name(&mut self, name: &str) -> bool {
        if let Some(idx) = self
            .themes
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(name))
        {
            self.theme_index = idx;
            true
        } else {
            self.theme_index = 0; // Fall back to Default
            false
        }
    }
}

/// Notification/alert to display
#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub is_error: bool,
    pub created_at: Instant,
}

/// Cached layout rectangles from the last render, used for mouse hit-testing.
/// Automatically updated every frame, so resize and visualiser toggle are handled.
#[derive(Debug, Clone, Default)]
pub struct LayoutAreas {
    pub header: Rect,
    pub content: Rect,
    pub now_playing: Rect,
    /// Left pane for dual-pane pages (Artists tree, Playlists list)
    pub content_left: Option<Rect>,
    /// Right pane for dual-pane pages (Songs list)
    pub content_right: Option<Rect>,
}

/// Mutable values produced during a render pass so drawing can run under a read lock.
///
/// Each scroll offset is `Option<usize>` so that only values actually touched by the
/// current page are written back, preventing cross-page state corruption.
#[derive(Debug, Default)]
pub struct RenderMutations {
    pub layout: LayoutAreas,
    pub browse_scroll_offset: Option<usize>,
    pub browse_album_scroll_offset: Option<usize>,
    pub queue_scroll_offset: Option<usize>,
    pub radio_scroll_offset: Option<usize>,
    pub playlists_playlist_scroll_offset: Option<usize>,
    pub playlists_song_scroll_offset: Option<usize>,
    pub artists_tree_scroll_offset: Option<usize>,
    pub artists_song_scroll_offset: Option<usize>,
    pub lyrics_scroll_offset: Option<usize>,
    pub lyrics_reset_manual_scroll: Option<bool>,
}

/// Complete application state
#[derive(Debug, Default)]
pub struct AppState {
    /// Application configuration
    pub config: Config,
    /// Current page
    pub page: Page,
    /// Now playing information
    pub now_playing: NowPlaying,
    /// Play queue (songs)
    pub queue: Vec<Child>,
    /// Current position in queue
    pub queue_position: Option<usize>,
    /// Playback volume (0-100), session-persistent across track changes
    pub volume: i32,
    /// Browse page state
    pub browse: BrowseState,
    /// Artists page state
    pub artists: ArtistsState,
    /// Queue page state
    pub queue_state: QueueState,
    /// Playlists page state
    pub playlists: PlaylistsState,
    /// Radio page state
    pub radio: RadioState,
    /// Server page state (connection settings)
    pub server_state: ServerState,
    /// Settings page state (app preferences)
    pub settings_state: SettingsState,
    /// Lyrics page state
    pub lyrics_state: LyricsState,
    /// Current notification
    pub notification: Option<Notification>,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Cava visualizer screen content (rows of styled spans)
    pub cava_screen: Vec<CavaRow>,
    /// Whether the cava binary is available on the system
    pub cava_available: bool,
    /// Cached layout areas from last render (for mouse hit-testing)
    pub layout: LayoutAreas,
}

/// A row of styled segments from cava's terminal output
#[derive(Debug, Clone, Default)]
pub struct CavaRow {
    pub spans: Vec<CavaSpan>,
}

/// A styled text segment from cava's terminal output
#[derive(Debug, Clone)]
pub struct CavaSpan {
    pub text: String,
    pub fg: CavaColor,
    pub bg: CavaColor,
}

/// Color from cava's terminal output
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CavaColor {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let mut state = Self {
            config: config.clone(),
            ..Default::default()
        };
        // Start on the first configured visible tab so hidden default pages
        // are not selected on launch.
        state.page = Page::visible_from_config(&config.visible_tabs)
            .first()
            .copied()
            .unwrap_or_default();
        // Initialize server page with current values
        state.server_state.base_url = config.base_url.clone();
        state.server_state.username = config.username.clone();
        state.server_state.password = config.password.clone();
        // Initialize cava from config
        state.settings_state.cava_enabled = config.cava;
        state.settings_state.cava_size = config.cava_size.clamp(10, 80);
        // Initialize notifications from config
        state.settings_state.notifications_enabled = config.notifications;
        // Initialize scrobbling from config
        state.settings_state.scrobble_enabled = config.scrobble;
        // Initialize save and restore queue from config
        state.settings_state.save_queue_enabled = config.save_queue;
        // Default to All songs so navigation and rendering start in sync
        state.browse.selected_option = Some(SongOption::All);
        // mpv starts at full volume; mirror that in state for the UI
        state.volume = 100;
        state.lyrics_state = LyricsState::default();

        state
    }

    /// Adjust the volume by `delta`, clamped to 0-100, returning the new value.
    pub fn adjust_volume(&mut self, delta: i32) -> i32 {
        self.volume = (self.volume + delta).clamp(0, 100);
        self.volume
    }

    /// Get the currently playing song from the queue
    pub fn current_song(&self) -> Option<&Child> {
        self.queue_position.and_then(|pos| self.queue.get(pos))
    }
    
    /// Show a notification
    pub fn notify(&mut self, message: impl Into<String>) {
        self.notification = Some(Notification {
            message: message.into(),
            is_error: false,
            created_at: Instant::now(),
        });
    }

    /// Show an error notification
    pub fn notify_error(&mut self, message: impl Into<String>) {
        self.notification = Some(Notification {
            message: message.into(),
            is_error: true,
            created_at: Instant::now(),
        });
    }

    /// Check if notification should be auto-cleared (after 2 seconds)
    pub fn check_notification_timeout(&mut self) {
        if let Some(ref notif) = self.notification {
            if notif.created_at.elapsed().as_secs() >= 2 {
                self.notification = None;
            }
        }
    }

    /// Clear the notification
    pub fn clear_notification(&mut self) {
        self.notification = None;
    }

    pub fn visible_pages(&self) -> Vec<Page> {
        Page::visible_from_config(&self.config.visible_tabs)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    use super::{AppState, Page};

    #[test]
    fn visible_pages_use_default_when_not_configured() {
        assert_eq!(Page::visible_from_config(&[]), Page::DEFAULT_TABS);
    }

    #[test]
    fn visible_pages_follow_config_order() {
        let names = vec![
            "Artists".to_string(),
            "Queue".to_string(),
            "Playlists".to_string(),
            "Browse".to_string(),
        ];

        assert_eq!(
            Page::visible_from_config(&names),
            vec![Page::Artists, Page::Queue, Page::Playlists, Page::Browse]
        );
    }

    #[test]
    fn visible_pages_ignore_unknown_and_duplicate_names() {
        let names = vec![
            "Queue".to_string(),
            "Nope".to_string(),
            "queue".to_string(),
            "Server".to_string(),
        ];

        assert_eq!(
            Page::visible_from_config(&names),
            vec![Page::Queue, Page::Server]
        );
    }

    #[test]
    fn app_state_starts_on_first_visible_tab() {
        let config = Config {
            visible_tabs: vec![
                "Artists".to_string(),
                "Queue".to_string(),
                "Server".to_string(),
            ],
            ..Default::default()
        };

        let state = AppState::new(config);

        assert_eq!(state.page, Page::Artists);
    }

    #[test]
    fn app_state_uses_default_page_when_visible_tabs_are_empty() {
        let state = AppState::new(Config::default());

        assert_eq!(state.page, Page::Browse);
    }

    #[test]
    fn app_state_starts_at_full_volume() {
        let state = AppState::new(Config::default());
        assert_eq!(state.volume, 100);
    }

    #[test]
    fn adjust_volume_clamps_to_0_100() {
        let mut state = AppState::new(Config::default());

        assert_eq!(state.adjust_volume(-5), 95);
        assert_eq!(state.adjust_volume(10), 100); // clamps at the top
        assert_eq!(state.adjust_volume(5), 100); // stays clamped

        // Drive it down past zero and confirm the floor.
        for _ in 0..25 {
            state.adjust_volume(-5);
        }
        assert_eq!(state.volume, 0);
        assert_eq!(state.adjust_volume(-5), 0);
    }
}

/// Thread-safe shared state
pub type SharedState = Arc<RwLock<AppState>>;

/// Create new shared state
pub fn new_shared_state(config: Config) -> SharedState {
    Arc::new(RwLock::new(AppState::new(config)))
}
