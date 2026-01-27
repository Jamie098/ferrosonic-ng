//! Main application module

#![allow(dead_code)]

pub mod actions;
pub mod state;

use std::io;
use std::io::Read as _;
use std::os::unix::io::FromRawFd;
use std::time::Duration;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::audio::mpv::MpvController;
use crate::audio::pipewire::PipeWireController;
use crate::config::Config;
use crate::error::{Error, UiError};
use crate::subsonic::SubsonicClient;
use crate::ui;

pub use actions::*;
pub use state::*;

/// Channel buffer size
const CHANNEL_SIZE: usize = 256;

/// Main application
pub struct App {
    /// Shared application state
    state: SharedState,
    /// Subsonic client
    subsonic: Option<SubsonicClient>,
    /// MPV audio controller
    mpv: MpvController,
    /// PipeWire sample rate controller
    pipewire: PipeWireController,
    /// Channel to send UI updates
    ui_tx: mpsc::Sender<UiAction>,
    /// Channel to receive UI updates
    ui_rx: mpsc::Receiver<UiAction>,
    /// Channel to send audio actions
    audio_tx: mpsc::Sender<AudioAction>,
    /// Channel to send subsonic actions
    subsonic_tx: mpsc::Sender<SubsonicAction>,
    /// Channel to send queue actions
    queue_tx: mpsc::Sender<QueueAction>,
    /// Cava child process
    cava_process: Option<std::process::Child>,
    /// Cava pty master fd for reading output
    cava_pty_master: Option<std::fs::File>,
    /// Cava terminal parser
    cava_parser: Option<vt100::Parser>,
    /// Last mouse click position and time (for second-click detection)
    last_click: Option<(u16, u16, std::time::Instant)>,
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Self {
        let (ui_tx, ui_rx) = mpsc::channel(CHANNEL_SIZE);
        let (audio_tx, _audio_rx) = mpsc::channel(CHANNEL_SIZE);
        let (subsonic_tx, _subsonic_rx) = mpsc::channel(CHANNEL_SIZE);
        let (queue_tx, _queue_rx) = mpsc::channel(CHANNEL_SIZE);

        let state = new_shared_state(config.clone());

        let subsonic = if config.is_configured() {
            match SubsonicClient::new(&config.base_url, &config.username, &config.password) {
                Ok(client) => Some(client),
                Err(e) => {
                    warn!("Failed to create Subsonic client: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            state,
            subsonic,
            mpv: MpvController::new(),
            pipewire: PipeWireController::new(),
            ui_tx,
            ui_rx,
            audio_tx,
            subsonic_tx,
            queue_tx,
            cava_process: None,
            cava_pty_master: None,
            cava_parser: None,
            last_click: None,
        }
    }

    /// Run the application
    pub async fn run(&mut self) -> Result<(), Error> {
        // Start MPV
        if let Err(e) = self.mpv.start() {
            warn!("Failed to start MPV: {} - audio playback won't work", e);
            let mut state = self.state.write().await;
            state.notify_error(format!("Failed to start MPV: {}. Is mpv installed?", e));
            drop(state);
        } else {
            info!("MPV started successfully, ready for playback");
        }

        // Seed and load themes
        {
            use crate::ui::theme::{load_themes, seed_default_themes};
            if let Some(themes_dir) = crate::config::paths::themes_dir() {
                seed_default_themes(&themes_dir);
            }
            let themes = load_themes();
            let mut state = self.state.write().await;
            let theme_name = state.config.theme.clone();
            state.settings_state.themes = themes;
            state.settings_state.set_theme_by_name(&theme_name);
        }

        // Check if cava is available
        let cava_available = std::process::Command::new("which")
            .arg("cava")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        {
            let mut state = self.state.write().await;
            state.cava_available = cava_available;
            if !cava_available {
                state.settings_state.cava_enabled = false;
            }
        }

        // Start cava if enabled and available
        {
            let state = self.state.read().await;
            if state.settings_state.cava_enabled && cava_available {
                let td = state.settings_state.current_theme();
                let g = td.cava_gradient.clone();
                let h = td.cava_horizontal_gradient.clone();
                drop(state);
                self.start_cava(&g, &h);
            }
        }

        // Setup terminal
        enable_raw_mode().map_err(UiError::TerminalInit)?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(UiError::TerminalInit)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).map_err(UiError::TerminalInit)?;

        info!("Terminal initialized");

        // Load initial data if configured
        if self.subsonic.is_some() {
            self.load_initial_data().await;
        }

        // Main event loop
        let result = self.event_loop(&mut terminal).await;

        // Cleanup cava
        self.stop_cava();

        // Cleanup MPV
        let _ = self.mpv.quit();

        // Cleanup terminal
        disable_raw_mode().map_err(UiError::TerminalInit)?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
            .map_err(UiError::TerminalInit)?;
        terminal.show_cursor().map_err(UiError::Render)?;

        info!("Terminal restored");
        result
    }

    /// Load initial data from server
    async fn load_initial_data(&mut self) {
        if let Some(ref client) = self.subsonic {
            // Load artists
            match client.get_artists().await {
                Ok(artists) => {
                    let mut state = self.state.write().await;
                    let count = artists.len();
                    state.artists.artists = artists;
                    // Select first artist by default
                    if count > 0 {
                        state.artists.selected_index = Some(0);
                    }
                    info!("Loaded {} artists", count);
                }
                Err(e) => {
                    error!("Failed to load artists: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to load artists: {}", e));
                }
            }

            // Load playlists
            match client.get_playlists().await {
                Ok(playlists) => {
                    let mut state = self.state.write().await;
                    let count = playlists.len();
                    state.playlists.playlists = playlists;
                    info!("Loaded {} playlists", count);
                }
                Err(e) => {
                    error!("Failed to load playlists: {}", e);
                    // Don't show error for playlists if artists loaded
                }
            }
        }
    }

    /// Main event loop
    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<(), Error> {
        let mut last_playback_update = std::time::Instant::now();

        loop {
            // Determine tick rate based on whether cava is active
            let cava_active = self.cava_parser.is_some();
            let tick_rate = if cava_active {
                Duration::from_millis(16) // ~60fps
            } else {
                Duration::from_millis(100)
            };

            // Draw UI
            {
                let mut state = self.state.write().await;
                terminal
                    .draw(|frame| ui::draw(frame, &mut state))
                    .map_err(UiError::Render)?;
            }

            // Check for quit
            {
                let state = self.state.read().await;
                if state.should_quit {
                    break;
                }
            }

            // Handle events with timeout
            if event::poll(tick_rate).map_err(UiError::Input)? {
                let event = event::read().map_err(UiError::Input)?;
                self.handle_event(event).await?;
            }

            // Process any pending UI actions
            while let Ok(action) = self.ui_rx.try_recv() {
                self.handle_ui_action(action).await;
            }

            // Read cava output (non-blocking)
            self.read_cava_output().await;

            // Update playback position every ~500ms
            let now = std::time::Instant::now();
            if now.duration_since(last_playback_update) >= Duration::from_millis(500) {
                last_playback_update = now;
                self.update_playback_info().await;
            }

            // Check for notification auto-clear (after 2 seconds)
            {
                let mut state = self.state.write().await;
                state.check_notification_timeout();
            }
        }

        Ok(())
    }

    /// Update playback position and audio info from MPV
    async fn update_playback_info(&mut self) {
        // Only update if something should be playing
        let state = self.state.read().await;
        let is_playing = state.now_playing.state == PlaybackState::Playing;
        let is_active = is_playing || state.now_playing.state == PlaybackState::Paused;
        drop(state);

        if !is_active || !self.mpv.is_running() {
            return;
        }

        // Check for track advancement
        if is_playing {
            // Early transition: if near end of track and no preloaded next track,
            // advance immediately instead of waiting for idle detection
            {
                let state = self.state.read().await;
                let time_remaining = state.now_playing.duration - state.now_playing.position;
                let has_next = state
                    .queue_position
                    .map(|p| p + 1 < state.queue.len())
                    .unwrap_or(false);
                drop(state);

                if has_next && time_remaining > 0.0 && time_remaining < 2.0 {
                    if let Ok(count) = self.mpv.get_playlist_count() {
                        if count < 2 {
                            info!("Near end of track with no preloaded next — advancing early");
                            let _ = self.next_track().await;
                            return;
                        }
                    }
                }
            }

            // Re-preload if the appended track was lost
            if let Ok(count) = self.mpv.get_playlist_count() {
                if count == 1 {
                    let state = self.state.read().await;
                    if let Some(pos) = state.queue_position {
                        if pos + 1 < state.queue.len() {
                            drop(state);
                            debug!("Playlist count is 1, re-preloading next track");
                            self.preload_next_track(pos).await;
                        }
                    }
                }
            }

            // Check if MPV advanced to next track in playlist (gapless transition)
            if let Ok(Some(mpv_pos)) = self.mpv.get_playlist_pos() {
                if mpv_pos == 1 {
                    // Gapless advance happened - update our state to match
                    let state = self.state.read().await;
                    if let Some(current_pos) = state.queue_position {
                        let next_pos = current_pos + 1;
                        if next_pos < state.queue.len() {
                            drop(state);
                            info!("Gapless advancement to track {}", next_pos);

                            // Update state - keep audio properties since they'll be similar
                            // for gapless transitions (same album, same format)
                            let mut state = self.state.write().await;
                            state.queue_position = Some(next_pos);
                            if let Some(song) = state.queue.get(next_pos).cloned() {
                                state.now_playing.song = Some(song.clone());
                                state.now_playing.position = 0.0;
                                state.now_playing.duration = song.duration.unwrap_or(0) as f64;
                                // Don't reset audio properties - let them update naturally
                                // This avoids triggering PipeWire rate changes unnecessarily
                            }
                            drop(state);

                            // Remove the finished track (index 0) from MPV's playlist
                            // This is less disruptive than playlist_clear during playback
                            let _ = self.mpv.playlist_remove(0);

                            // Preload the next track for continued gapless playback
                            self.preload_next_track(next_pos).await;
                            return;
                        }
                    }
                    drop(state);
                }
            }

            // Check if MPV went idle (track ended, no preloaded track)
            if let Ok(idle) = self.mpv.is_idle() {
                if idle {
                    info!("Track ended, advancing to next");
                    let _ = self.next_track().await;
                    return;
                }
            }
        }

        // Get position from MPV
        if let Ok(position) = self.mpv.get_time_pos() {
            let mut state = self.state.write().await;
            state.now_playing.position = position;
        }

        // Get duration if not set
        {
            let state = self.state.read().await;
            if state.now_playing.duration <= 0.0 {
                drop(state);
                if let Ok(duration) = self.mpv.get_duration() {
                    if duration > 0.0 {
                        let mut state = self.state.write().await;
                        state.now_playing.duration = duration;
                    }
                }
            }
        }

        // Get audio properties - keep polling until we get valid values
        // MPV may not have them ready immediately when playback starts
        {
            let state = self.state.read().await;
            let need_sample_rate = state.now_playing.sample_rate.is_none();
            drop(state);

            if need_sample_rate {
                // Try to get audio properties from MPV
                let sample_rate = self.mpv.get_sample_rate().ok().flatten();
                let bit_depth = self.mpv.get_bit_depth().ok().flatten();
                let format = self.mpv.get_audio_format().ok().flatten();
                let channels = self.mpv.get_channels().ok().flatten();

                // Only update if we got a valid sample rate (indicates audio is ready)
                if let Some(rate) = sample_rate {
                    // Only switch PipeWire sample rate if it's actually different
                    // This avoids unnecessary rate switches during gapless playback
                    // of albums with the same sample rate
                    let current_pw_rate = self.pipewire.get_current_rate();
                    if current_pw_rate != Some(rate) {
                        info!("Sample rate change: {:?} -> {} Hz", current_pw_rate, rate);
                        if let Err(e) = self.pipewire.set_rate(rate) {
                            warn!("Failed to set PipeWire sample rate: {}", e);
                        }
                    } else {
                        debug!(
                            "Sample rate unchanged at {} Hz, skipping PipeWire switch",
                            rate
                        );
                    }

                    let mut state = self.state.write().await;
                    state.now_playing.sample_rate = Some(rate);
                    state.now_playing.bit_depth = bit_depth;
                    state.now_playing.format = format;
                    state.now_playing.channels = channels;
                }
            }
        }
    }

    /// Handle terminal events
    async fn handle_event(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::Key(key) => {
                // Only handle key press events, ignore release and repeat
                if key.kind == event::KeyEventKind::Press {
                    self.handle_key(key).await
                } else {
                    Ok(())
                }
            }
            Event::Mouse(mouse) => self.handle_mouse(mouse).await,
            Event::Resize(_, _) => {
                // Restart cava so it picks up the new terminal dimensions
                if self.cava_parser.is_some() {
                    let state = self.state.read().await;
                    let td = state.settings_state.current_theme();
                    let g = td.cava_gradient.clone();
                    let h = td.cava_horizontal_gradient.clone();
                    drop(state);
                    self.start_cava(&g, &h);
                    let mut state = self.state.write().await;
                    state.cava_screen.clear();
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Handle keyboard input
    async fn handle_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut state = self.state.write().await;

        // Clear notification on any keypress
        state.clear_notification();

        // Bypass global keybindings when typing in server text fields or filtering artists
        let is_server_text_field =
            state.page == Page::Server && state.server_state.selected_field <= 2;
        let is_filtering = state.page == Page::Artists && state.artists.filter_active;

        if is_server_text_field || is_filtering {
            let page = state.page;
            drop(state);
            return match page {
                Page::Server => self.handle_server_key(key).await,
                Page::Artists => self.handle_artists_key(key).await,
                _ => Ok(()),
            };
        }

        // Global keybindings
        match (key.code, key.modifiers) {
            // Quit
            (KeyCode::Char('q'), KeyModifiers::NONE) => {
                state.should_quit = true;
                return Ok(());
            }
            // Page switching
            (KeyCode::F(1), _) => {
                state.page = Page::Artists;
                return Ok(());
            }
            (KeyCode::F(2), _) => {
                state.page = Page::Queue;
                return Ok(());
            }
            (KeyCode::F(3), _) => {
                state.page = Page::Playlists;
                return Ok(());
            }
            (KeyCode::F(4), _) => {
                state.page = Page::Server;
                return Ok(());
            }
            (KeyCode::F(5), _) => {
                state.page = Page::Settings;
                return Ok(());
            }
            // Playback controls (global)
            (KeyCode::Char('p'), KeyModifiers::NONE) | (KeyCode::Char(' '), KeyModifiers::NONE) => {
                // Toggle pause
                drop(state);
                return self.toggle_pause().await;
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                // Next track
                drop(state);
                return self.next_track().await;
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) => {
                // Previous track
                drop(state);
                return self.prev_track().await;
            }
            // Cycle theme (global)
            (KeyCode::Char('t'), KeyModifiers::NONE) => {
                state.settings_state.next_theme();
                state.config.theme = state.settings_state.theme_name().to_string();
                let label = state.settings_state.theme_name().to_string();
                state.notify(format!("Theme: {}", label));
                let _ = state.config.save_default();
                let cava_enabled = state.settings_state.cava_enabled;
                let td = state.settings_state.current_theme();
                let g = td.cava_gradient.clone();
                let h = td.cava_horizontal_gradient.clone();
                drop(state);
                if cava_enabled {
                    self.start_cava(&g, &h);
                }
                return Ok(());
            }
            // Ctrl+R to refresh data from server
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                state.notify("Refreshing...");
                drop(state);
                self.load_initial_data().await;
                let mut state = self.state.write().await;
                state.notify("Data refreshed");
                return Ok(());
            }
            _ => {}
        }

        // Page-specific keybindings
        let page = state.page;
        drop(state);
        match page {
            Page::Artists => self.handle_artists_key(key).await,
            Page::Queue => self.handle_queue_key(key).await,
            Page::Playlists => self.handle_playlists_key(key).await,
            Page::Server => self.handle_server_key(key).await,
            Page::Settings => self.handle_settings_key(key).await,
        }
    }

    /// Handle artists page keys
    async fn handle_artists_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        use crate::ui::pages::artists::{build_tree_items, TreeItem};

        let mut state = self.state.write().await;

        // Handle filter input mode
        if state.artists.filter_active {
            match key.code {
                KeyCode::Esc => {
                    state.artists.filter_active = false;
                    state.artists.filter.clear();
                }
                KeyCode::Enter => {
                    state.artists.filter_active = false;
                }
                KeyCode::Backspace => {
                    state.artists.filter.pop();
                }
                KeyCode::Char(c) => {
                    state.artists.filter.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('/') => {
                state.artists.filter_active = true;
            }
            KeyCode::Esc => {
                state.artists.filter.clear();
                state.artists.expanded.clear();
                state.artists.selected_index = Some(0);
            }
            KeyCode::Tab => {
                state.artists.focus = (state.artists.focus + 1) % 2;
            }
            KeyCode::Left => {
                state.artists.focus = 0;
            }
            KeyCode::Right => {
                // Move focus to songs (right pane)
                if !state.artists.songs.is_empty() {
                    state.artists.focus = 1;
                    if state.artists.selected_song.is_none() {
                        state.artists.selected_song = Some(0);
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if state.artists.focus == 0 {
                    // Tree navigation
                    let tree_items = build_tree_items(&state);
                    if let Some(sel) = state.artists.selected_index {
                        if sel > 0 {
                            state.artists.selected_index = Some(sel - 1);
                        }
                    } else if !tree_items.is_empty() {
                        state.artists.selected_index = Some(0);
                    }
                    // Preview album songs in right pane
                    let album_id = state
                        .artists
                        .selected_index
                        .and_then(|i| tree_items.get(i))
                        .and_then(|item| match item {
                            TreeItem::Album { album } => Some(album.id.clone()),
                            _ => None,
                        });
                    if let Some(album_id) = album_id {
                        drop(state);
                        if let Some(ref client) = self.subsonic {
                            if let Ok((_album, songs)) = client.get_album(&album_id).await {
                                let mut state = self.state.write().await;
                                state.artists.songs = songs;
                                state.artists.selected_song = Some(0);
                            }
                        }
                        return Ok(());
                    }
                } else {
                    // Song list
                    if let Some(sel) = state.artists.selected_song {
                        if sel > 0 {
                            state.artists.selected_song = Some(sel - 1);
                        }
                    } else if !state.artists.songs.is_empty() {
                        state.artists.selected_song = Some(0);
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if state.artists.focus == 0 {
                    // Tree navigation
                    let tree_items = build_tree_items(&state);
                    let max = tree_items.len().saturating_sub(1);
                    if let Some(sel) = state.artists.selected_index {
                        if sel < max {
                            state.artists.selected_index = Some(sel + 1);
                        }
                    } else if !tree_items.is_empty() {
                        state.artists.selected_index = Some(0);
                    }
                    // Preview album songs in right pane
                    let album_id = state
                        .artists
                        .selected_index
                        .and_then(|i| tree_items.get(i))
                        .and_then(|item| match item {
                            TreeItem::Album { album } => Some(album.id.clone()),
                            _ => None,
                        });
                    if let Some(album_id) = album_id {
                        drop(state);
                        if let Some(ref client) = self.subsonic {
                            if let Ok((_album, songs)) = client.get_album(&album_id).await {
                                let mut state = self.state.write().await;
                                state.artists.songs = songs;
                                state.artists.selected_song = Some(0);
                            }
                        }
                        return Ok(());
                    }
                } else {
                    // Song list
                    let max = state.artists.songs.len().saturating_sub(1);
                    if let Some(sel) = state.artists.selected_song {
                        if sel < max {
                            state.artists.selected_song = Some(sel + 1);
                        }
                    } else if !state.artists.songs.is_empty() {
                        state.artists.selected_song = Some(0);
                    }
                }
            }
            KeyCode::Enter => {
                if state.artists.focus == 0 {
                    // Get current tree item
                    let tree_items = build_tree_items(&state);
                    if let Some(idx) = state.artists.selected_index {
                        if let Some(item) = tree_items.get(idx) {
                            match item {
                                TreeItem::Artist { artist, expanded } => {
                                    let artist_id = artist.id.clone();
                                    let artist_name = artist.name.clone();
                                    let was_expanded = *expanded;

                                    if was_expanded {
                                        state.artists.expanded.remove(&artist_id);
                                    } else {
                                        if !state.artists.albums_cache.contains_key(&artist_id) {
                                            drop(state);
                                            if let Some(ref client) = self.subsonic {
                                                match client.get_artist(&artist_id).await {
                                                    Ok((_artist, albums)) => {
                                                        let mut state = self.state.write().await;
                                                        let count = albums.len();
                                                        state.artists.albums_cache.insert(artist_id.clone(), albums);
                                                        state.artists.expanded.insert(artist_id);
                                                        info!("Loaded {} albums for {}", count, artist_name);
                                                    }
                                                    Err(e) => {
                                                        let mut state = self.state.write().await;
                                                        state.notify_error(format!("Failed to load: {}", e));
                                                    }
                                                }
                                            }
                                            return Ok(());
                                        } else {
                                            state.artists.expanded.insert(artist_id);
                                        }
                                    }
                                }
                                TreeItem::Album { album } => {
                                    let album_id = album.id.clone();
                                    let album_name = album.name.clone();
                                    drop(state);

                                    if let Some(ref client) = self.subsonic {
                                        match client.get_album(&album_id).await {
                                            Ok((_album, songs)) => {
                                                if songs.is_empty() {
                                                    let mut state = self.state.write().await;
                                                    state.notify_error("Album has no songs");
                                                    return Ok(());
                                                }

                                                let first_song = songs[0].clone();
                                                let stream_url = client.get_stream_url(&first_song.id);

                                                let mut state = self.state.write().await;
                                                let count = songs.len();
                                                state.queue.clear();
                                                state.queue.extend(songs.clone());
                                                state.queue_position = Some(0);
                                                state.artists.songs = songs;
                                                state.artists.selected_song = Some(0);
                                                state.artists.focus = 1;
                                                state.now_playing.song = Some(first_song.clone());
                                                state.now_playing.state = PlaybackState::Playing;
                                                state.now_playing.position = 0.0;
                                                state.now_playing.duration = first_song.duration.unwrap_or(0) as f64;
                                                state.now_playing.sample_rate = None;
                                                state.now_playing.bit_depth = None;
                                                state.now_playing.format = None;
                                                state.now_playing.channels = None;
                                                state.notify(format!("Playing album: {} ({} songs)", album_name, count));
                                                drop(state);

                                                match stream_url {
                                                    Ok(url) => {
                                                        if self.mpv.is_paused().unwrap_or(false) {
                                                            let _ = self.mpv.resume();
                                                        }
                                                        if let Err(e) = self.mpv.loadfile(&url) {
                                                            error!("Failed to play: {}", e);
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to get stream URL: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let mut state = self.state.write().await;
                                                state.notify_error(format!("Failed to load album: {}", e));
                                            }
                                        }
                                    }
                                    return Ok(());
                                }
                            }
                        }
                    }
                } else {
                    // Play selected song from current position
                    if let Some(idx) = state.artists.selected_song {
                        if idx < state.artists.songs.len() {
                            let song = state.artists.songs[idx].clone();
                            let songs = state.artists.songs.clone();
                            state.queue.clear();
                            state.queue.extend(songs);
                            state.queue_position = Some(idx);
                            state.now_playing.song = Some(song.clone());
                            state.now_playing.state = PlaybackState::Playing;
                            state.now_playing.position = 0.0;
                            state.now_playing.duration = song.duration.unwrap_or(0) as f64;
                            state.now_playing.sample_rate = None;
                            state.now_playing.bit_depth = None;
                            state.now_playing.format = None;
                            state.now_playing.channels = None;
                            state.notify(format!("Playing: {}", song.title));
                            drop(state);

                            if let Some(ref client) = self.subsonic {
                                match client.get_stream_url(&song.id) {
                                    Ok(url) => {
                                        if self.mpv.is_paused().unwrap_or(false) {
                                            let _ = self.mpv.resume();
                                        }
                                        if let Err(e) = self.mpv.loadfile(&url) {
                                            error!("Failed to play: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to get stream URL: {}", e);
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                if state.artists.focus == 1 {
                    state.artists.focus = 0;
                }
            }
            KeyCode::Char('e') => {
                if state.artists.focus == 1 {
                    if let Some(idx) = state.artists.selected_song {
                        if let Some(song) = state.artists.songs.get(idx).cloned() {
                            let title = song.title.clone();
                            state.queue.push(song);
                            state.notify(format!("Added to queue: {}", title));
                        }
                    }
                } else {
                    if !state.artists.songs.is_empty() {
                        let count = state.artists.songs.len();
                        let songs = state.artists.songs.clone();
                        state.queue.extend(songs);
                        state.notify(format!("Added {} songs to queue", count));
                    }
                }
            }
            KeyCode::Char('n') => {
                let insert_pos = state.queue_position.map(|p| p + 1).unwrap_or(0);
                if state.artists.focus == 1 {
                    if let Some(idx) = state.artists.selected_song {
                        if let Some(song) = state.artists.songs.get(idx).cloned() {
                            let title = song.title.clone();
                            state.queue.insert(insert_pos, song);
                            state.notify(format!("Playing next: {}", title));
                        }
                    }
                } else {
                    if !state.artists.songs.is_empty() {
                        let count = state.artists.songs.len();
                        let songs: Vec<_> = state.artists.songs.iter().cloned().collect();
                        for (i, song) in songs.into_iter().enumerate() {
                            state.queue.insert(insert_pos + i, song);
                        }
                        state.notify(format!("Playing {} songs next", count));
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle queue page keys
    async fn handle_queue_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut state = self.state.write().await;

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(sel) = state.queue_state.selected {
                    if sel > 0 {
                        state.queue_state.selected = Some(sel - 1);
                    }
                } else if !state.queue.is_empty() {
                    state.queue_state.selected = Some(0);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = state.queue.len().saturating_sub(1);
                if let Some(sel) = state.queue_state.selected {
                    if sel < max {
                        state.queue_state.selected = Some(sel + 1);
                    }
                } else if !state.queue.is_empty() {
                    state.queue_state.selected = Some(0);
                }
            }
            KeyCode::Enter => {
                // Play selected song
                if let Some(idx) = state.queue_state.selected {
                    if idx < state.queue.len() {
                        drop(state);
                        return self.play_queue_position(idx).await;
                    }
                }
            }
            KeyCode::Char('d') => {
                // Remove selected song
                if let Some(idx) = state.queue_state.selected {
                    if idx < state.queue.len() {
                        let song = state.queue.remove(idx);
                        state.notify(format!("Removed: {}", song.title));
                        // Adjust selection
                        if state.queue.is_empty() {
                            state.queue_state.selected = None;
                        } else if idx >= state.queue.len() {
                            state.queue_state.selected = Some(state.queue.len() - 1);
                        }
                        // Adjust queue position
                        if let Some(pos) = state.queue_position {
                            if idx < pos {
                                state.queue_position = Some(pos - 1);
                            } else if idx == pos {
                                state.queue_position = None;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('J') => {
                // Move down
                if let Some(idx) = state.queue_state.selected {
                    if idx < state.queue.len() - 1 {
                        state.queue.swap(idx, idx + 1);
                        state.queue_state.selected = Some(idx + 1);
                        // Adjust queue position if needed
                        if let Some(pos) = state.queue_position {
                            if pos == idx {
                                state.queue_position = Some(idx + 1);
                            } else if pos == idx + 1 {
                                state.queue_position = Some(idx);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('K') => {
                // Move up
                if let Some(idx) = state.queue_state.selected {
                    if idx > 0 {
                        state.queue.swap(idx, idx - 1);
                        state.queue_state.selected = Some(idx - 1);
                        // Adjust queue position if needed
                        if let Some(pos) = state.queue_position {
                            if pos == idx {
                                state.queue_position = Some(idx - 1);
                            } else if pos == idx - 1 {
                                state.queue_position = Some(idx);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                // Shuffle queue
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();

                if let Some(pos) = state.queue_position {
                    // Keep current song in place, shuffle the rest
                    if pos < state.queue.len() {
                        let current = state.queue.remove(pos);
                        state.queue.shuffle(&mut rng);
                        state.queue.insert(0, current);
                        state.queue_position = Some(0);
                    }
                } else {
                    state.queue.shuffle(&mut rng);
                }
                state.notify("Queue shuffled");
            }
            KeyCode::Char('c') => {
                // Clear history (remove all songs before current position)
                if let Some(pos) = state.queue_position {
                    if pos > 0 {
                        let removed = pos;
                        state.queue.drain(0..pos);
                        state.queue_position = Some(0);
                        // Adjust selection
                        if let Some(sel) = state.queue_state.selected {
                            if sel < pos {
                                state.queue_state.selected = Some(0);
                            } else {
                                state.queue_state.selected = Some(sel - pos);
                            }
                        }
                        state.notify(format!("Cleared {} played songs", removed));
                    } else {
                        state.notify("No history to clear");
                    }
                } else {
                    state.notify("No history to clear");
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle playlists page keys
    async fn handle_playlists_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut state = self.state.write().await;

        match key.code {
            KeyCode::Tab => {
                state.playlists.focus = (state.playlists.focus + 1) % 2;
            }
            KeyCode::Left => {
                state.playlists.focus = 0;
            }
            KeyCode::Right => {
                if !state.playlists.songs.is_empty() {
                    state.playlists.focus = 1;
                    if state.playlists.selected_song.is_none() {
                        state.playlists.selected_song = Some(0);
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if state.playlists.focus == 0 {
                    // Playlist list
                    if let Some(sel) = state.playlists.selected_playlist {
                        if sel > 0 {
                            state.playlists.selected_playlist = Some(sel - 1);
                        }
                    } else if !state.playlists.playlists.is_empty() {
                        state.playlists.selected_playlist = Some(0);
                    }
                } else {
                    // Song list
                    if let Some(sel) = state.playlists.selected_song {
                        if sel > 0 {
                            state.playlists.selected_song = Some(sel - 1);
                        }
                    } else if !state.playlists.songs.is_empty() {
                        state.playlists.selected_song = Some(0);
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if state.playlists.focus == 0 {
                    let max = state.playlists.playlists.len().saturating_sub(1);
                    if let Some(sel) = state.playlists.selected_playlist {
                        if sel < max {
                            state.playlists.selected_playlist = Some(sel + 1);
                        }
                    } else if !state.playlists.playlists.is_empty() {
                        state.playlists.selected_playlist = Some(0);
                    }
                } else {
                    let max = state.playlists.songs.len().saturating_sub(1);
                    if let Some(sel) = state.playlists.selected_song {
                        if sel < max {
                            state.playlists.selected_song = Some(sel + 1);
                        }
                    } else if !state.playlists.songs.is_empty() {
                        state.playlists.selected_song = Some(0);
                    }
                }
            }
            KeyCode::Enter => {
                if state.playlists.focus == 0 {
                    // Load playlist songs
                    if let Some(idx) = state.playlists.selected_playlist {
                        if let Some(playlist) = state.playlists.playlists.get(idx) {
                            let playlist_id = playlist.id.clone();
                            let playlist_name = playlist.name.clone();
                            drop(state);

                            if let Some(ref client) = self.subsonic {
                                match client.get_playlist(&playlist_id).await {
                                    Ok((_playlist, songs)) => {
                                        let mut state = self.state.write().await;
                                        let count = songs.len();
                                        state.playlists.songs = songs;
                                        state.playlists.selected_song =
                                            if count > 0 { Some(0) } else { None };
                                        state.playlists.focus = 1;
                                        state.notify(format!(
                                                "Loaded playlist: {} ({} songs)",
                                                playlist_name, count
                                        ));
                                    }
                                    Err(e) => {
                                        let mut state = self.state.write().await;
                                        state.notify_error(format!(
                                                "Failed to load playlist: {}",
                                                e
                                        ));
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }
                } else {
                    // Play selected song from playlist
                    if let Some(idx) = state.playlists.selected_song {
                        if idx < state.playlists.songs.len() {
                            let songs = state.playlists.songs.clone();
                            state.queue.clear();
                            state.queue.extend(songs);
                            drop(state);
                            return self.play_queue_position(idx).await;
                        }
                    }
                }
            }
            KeyCode::Char('e') => {
                // Add to queue
                if state.playlists.focus == 1 {
                    if let Some(idx) = state.playlists.selected_song {
                        if let Some(song) = state.playlists.songs.get(idx).cloned() {
                            let title = song.title.clone();
                            state.queue.push(song);
                            state.notify(format!("Added to queue: {}", title));
                        }
                    }
                } else {
                    // Add whole playlist
                    if !state.playlists.songs.is_empty() {
                        let count = state.playlists.songs.len();
                        let songs = state.playlists.songs.clone();
                        state.queue.extend(songs);
                        state.notify(format!("Added {} songs to queue", count));
                    }
                }
            }
            KeyCode::Char('n') => {
                // Add next
                let insert_pos = state.queue_position.map(|p| p + 1).unwrap_or(0);
                if state.playlists.focus == 1 {
                    if let Some(idx) = state.playlists.selected_song {
                        if let Some(song) = state.playlists.songs.get(idx).cloned() {
                            let title = song.title.clone();
                            state.queue.insert(insert_pos, song);
                            state.notify(format!("Playing next: {}", title));
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                // Shuffle play playlist
                use rand::seq::SliceRandom;
                if !state.playlists.songs.is_empty() {
                    let mut songs = state.playlists.songs.clone();
                    songs.shuffle(&mut rand::thread_rng());
                    state.queue.clear();
                    state.queue.extend(songs);
                    drop(state);
                    return self.play_queue_position(0).await;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle server page keys
    async fn handle_server_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut state = self.state.write().await;

        let field = state.server_state.selected_field;
        let is_text_field = field <= 2;

        match key.code {
            // Navigation - always works
            KeyCode::Up => {
                if field > 0 {
                    state.server_state.selected_field -= 1;
                }
            }
            KeyCode::Down => {
                if field < 4 {
                    state.server_state.selected_field += 1;
                }
            }
            KeyCode::Tab => {
                // Tab moves to next field, wrapping around
                state.server_state.selected_field = (field + 1) % 5;
            }
            // Text input for text fields (0=URL, 1=Username, 2=Password)
            KeyCode::Char(c) if is_text_field => match field {
                0 => state.server_state.base_url.push(c),
                1 => state.server_state.username.push(c),
                2 => state.server_state.password.push(c),
                _ => {}
            },
            KeyCode::Backspace if is_text_field => match field {
                0 => {
                    state.server_state.base_url.pop();
                }
                1 => {
                    state.server_state.username.pop();
                }
                2 => {
                    state.server_state.password.pop();
                }
                _ => {}
            },
            // Enter activates buttons, ignored on text fields
            KeyCode::Enter => {
                match field {
                    3 => {
                        // Test connection
                        let url = state.server_state.base_url.clone();
                        let user = state.server_state.username.clone();
                        let pass = state.server_state.password.clone();
                        state.server_state.status = Some("Testing connection...".to_string());
                        drop(state);

                        match SubsonicClient::new(&url, &user, &pass) {
                            Ok(client) => match client.ping().await {
                                Ok(_) => {
                                    let mut state = self.state.write().await;
                                    state.server_state.status =
                                        Some("Connection successful!".to_string());
                                }
                                Err(e) => {
                                    let mut state = self.state.write().await;
                                    state.server_state.status =
                                        Some(format!("Connection failed: {}", e));
                                }
                            },
                            Err(e) => {
                                let mut state = self.state.write().await;
                                state.server_state.status = Some(format!("Invalid URL: {}", e));
                            }
                        }
                        return Ok(());
                    }
                    4 => {
                        // Save config and reconnect
                        info!(
                            "Saving config: url='{}', user='{}'",
                            state.server_state.base_url, state.server_state.username
                        );
                        state.config.base_url = state.server_state.base_url.clone();
                        state.config.username = state.server_state.username.clone();
                        state.config.password = state.server_state.password.clone();

                        let url = state.config.base_url.clone();
                        let user = state.config.username.clone();
                        let pass = state.config.password.clone();

                        match state.config.save_default() {
                            Ok(_) => {
                                info!("Config saved successfully");
                                state.server_state.status =
                                    Some("Saved! Connecting...".to_string());
                            }
                            Err(e) => {
                                info!("Config save failed: {}", e);
                                state.server_state.status = Some(format!("Save failed: {}", e));
                                return Ok(());
                            }
                        }
                        drop(state);

                        // Create new client and load data
                        match SubsonicClient::new(&url, &user, &pass) {
                            Ok(client) => {
                                self.subsonic = Some(client);
                                self.load_initial_data().await;
                                let mut state = self.state.write().await;
                                state.server_state.status =
                                    Some("Connected and loaded data!".to_string());
                            }
                            Err(e) => {
                                let mut state = self.state.write().await;
                                state.server_state.status =
                                    Some(format!("Saved but connection failed: {}", e));
                            }
                        }
                        return Ok(());
                    }
                    _ => {} // Ignore Enter on text fields (handles paste with newlines)
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Start cava process in noncurses mode via a pty
    fn start_cava(&mut self, cava_gradient: &[String; 8], cava_horizontal_gradient: &[String; 8]) {
        self.stop_cava();

        // Compute pty dimensions to match the cava widget area
        let (term_w, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
        let cava_h = (term_h as u32 * 40 / 100).max(4) as u16;
        let cava_w = term_w;

        // Open a pty pair
        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;
        unsafe {
            if libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) != 0
            {
                error!("openpty failed");
                return;
            }

            // Set pty size so cava knows its dimensions
            let ws = libc::winsize {
                ws_row: cava_h,
                ws_col: cava_w,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            libc::ioctl(slave, libc::TIOCSWINSZ, &ws);
        }

        // Generate themed cava config and write to temp file
        // Dup slave fd before converting to File (from_raw_fd takes ownership)
        let slave_stdin_fd = unsafe { libc::dup(slave) };
        let slave_stderr_fd = unsafe { libc::dup(slave) };
        let slave_stdout = unsafe { std::fs::File::from_raw_fd(slave) };
        let slave_stdin = unsafe { std::fs::File::from_raw_fd(slave_stdin_fd) };
        let slave_stderr = unsafe { std::fs::File::from_raw_fd(slave_stderr_fd) };
        let config_path = std::env::temp_dir().join("ferrosonic-cava.conf");
        if let Err(e) = std::fs::write(&config_path, generate_cava_config(cava_gradient, cava_horizontal_gradient)) {
            error!("Failed to write cava config: {}", e);
            return;
        }
        let mut cmd = std::process::Command::new("cava");
        cmd.arg("-p").arg(&config_path);
        cmd.stdout(std::process::Stdio::from(slave_stdout))
            .stderr(std::process::Stdio::from(slave_stderr))
            .stdin(std::process::Stdio::from(slave_stdin))
            .env("TERM", "xterm-256color");

        match cmd.spawn() {
            Ok(child) => {
                // Set master to non-blocking
                unsafe {
                    let flags = libc::fcntl(master, libc::F_GETFL);
                    libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }

                let master_file = unsafe { std::fs::File::from_raw_fd(master) };
                let parser = vt100::Parser::new(cava_h, cava_w, 0);

                self.cava_process = Some(child);
                self.cava_pty_master = Some(master_file);
                self.cava_parser = Some(parser);
                info!("Cava started in noncurses mode ({}x{})", cava_w, cava_h);
            }
            Err(e) => {
                error!("Failed to start cava: {}", e);
                unsafe {
                    libc::close(master);
                }
            }
        }
    }

    /// Stop cava process and clean up
    fn stop_cava(&mut self) {
        if let Some(ref mut child) = self.cava_process {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.cava_process = None;
        self.cava_pty_master = None;
        self.cava_parser = None;
    }

    /// Read cava pty output and snapshot screen to state
    async fn read_cava_output(&mut self) {
        let (Some(ref mut master), Some(ref mut parser)) =
            (&mut self.cava_pty_master, &mut self.cava_parser)
            else {
                return;
            };

            // Read all available bytes from the pty master
            let mut buf = [0u8; 16384];
            let mut got_data = false;
            loop {
                match master.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        parser.process(&buf[..n]);
                        got_data = true;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => return,
                }
            }

            if !got_data {
                return;
            }

            // Snapshot the vt100 screen into shared state
            let screen = parser.screen();
            let (rows, cols) = screen.size();
            let mut cava_screen = Vec::with_capacity(rows as usize);

            for row in 0..rows {
                let mut spans: Vec<CavaSpan> = Vec::new();
                let mut cur_text = String::new();
                let mut cur_fg = CavaColor::Default;
                let mut cur_bg = CavaColor::Default;

                for col in 0..cols {
                    let cell = screen.cell(row, col).unwrap();
                    let fg = vt100_color_to_cava(cell.fgcolor());
                    let bg = vt100_color_to_cava(cell.bgcolor());

                    if fg != cur_fg || bg != cur_bg {
                        if !cur_text.is_empty() {
                            spans.push(CavaSpan {
                                text: std::mem::take(&mut cur_text),
                                fg: cur_fg,
                                bg: cur_bg,
                            });
                        }
                        cur_fg = fg;
                        cur_bg = bg;
                    }

                    let contents = cell.contents();
                    if contents.is_empty() {
                        cur_text.push(' ');
                    } else {
                        cur_text.push_str(&contents);
                    }
                }
                if !cur_text.is_empty() {
                    spans.push(CavaSpan {
                        text: cur_text,
                        fg: cur_fg,
                        bg: cur_bg,
                    });
                }
                cava_screen.push(CavaRow { spans });
            }

            let mut state = self.state.write().await;
            state.cava_screen = cava_screen;
    }

    /// Handle settings page keys
    async fn handle_settings_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut config_changed = false;

        {
            let mut state = self.state.write().await;
            let field = state.settings_state.selected_field;

            match key.code {
                // Navigate between fields
                KeyCode::Up | KeyCode::Char('k') => {
                    if field > 0 {
                        state.settings_state.selected_field = field - 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if field < 1 {
                        state.settings_state.selected_field = field + 1;
                    }
                }
                // Left
                KeyCode::Left | KeyCode::Char('h') => match field {
                    0 => {
                        state.settings_state.prev_theme();
                        state.config.theme = state.settings_state.theme_name().to_string();
                        let label = state.settings_state.theme_name().to_string();
                        state.notify(format!("Theme: {}", label));
                        config_changed = true;
                    }
                    1 if state.cava_available => {
                        state.settings_state.cava_enabled = !state.settings_state.cava_enabled;
                        state.config.cava = state.settings_state.cava_enabled;
                        let status = if state.settings_state.cava_enabled {
                            "On"
                        } else {
                            "Off"
                        };
                        state.notify(format!("Cava: {}", status));
                        config_changed = true;
                    }
                    _ => {}
                },
                // Right / Enter / Space
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                    match field {
                        0 => {
                            state.settings_state.next_theme();
                            state.config.theme = state.settings_state.theme_name().to_string();
                            let label = state.settings_state.theme_name().to_string();
                            state.notify(format!("Theme: {}", label));
                            config_changed = true;
                        }
                        1 if state.cava_available => {
                            state.settings_state.cava_enabled = !state.settings_state.cava_enabled;
                            state.config.cava = state.settings_state.cava_enabled;
                            let status = if state.settings_state.cava_enabled {
                                "On"
                            } else {
                                "Off"
                            };
                            state.notify(format!("Cava: {}", status));
                            config_changed = true;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if config_changed {
            // Save config
            let state = self.state.read().await;
            if let Err(e) = state.config.save_default() {
                drop(state);
                let mut state = self.state.write().await;
                state.notify_error(format!("Failed to save: {}", e));
            } else {
                // Start/stop cava based on new setting, or restart on theme change
                let cava_enabled = state.settings_state.cava_enabled;
                let td = state.settings_state.current_theme();
                let g = td.cava_gradient.clone();
                let h = td.cava_horizontal_gradient.clone();
                let cava_running = self.cava_parser.is_some();
                drop(state);
                if cava_enabled {
                    // (Re)start cava — picks up new theme colors or toggle-on
                    self.start_cava(&g, &h);
                } else if cava_running {
                    self.stop_cava();
                    let mut state = self.state.write().await;
                    state.cava_screen.clear();
                }
            }
        }

        Ok(())
    }

    /// Handle mouse input
    async fn handle_mouse(&mut self, mouse: event::MouseEvent) -> Result<(), Error> {
        let x = mouse.column;
        let y = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(x, y).await
            }
            MouseEventKind::ScrollUp => {
                self.handle_mouse_scroll_up().await
            }
            MouseEventKind::ScrollDown => {
                self.handle_mouse_scroll_down().await
            }
            _ => Ok(()),
        }
    }

    /// Handle left mouse click
    async fn handle_mouse_click(&mut self, x: u16, y: u16) -> Result<(), Error> {
        use crate::ui::header::{Header, HeaderRegion};

        let state = self.state.read().await;
        let layout = state.layout.clone();
        let page = state.page;
        let duration = state.now_playing.duration;
        drop(state);

        // Check header area
        if y >= layout.header.y && y < layout.header.y + layout.header.height {
            if let Some(region) = Header::region_at(layout.header, x, y) {
                match region {
                    HeaderRegion::Tab(tab_page) => {
                        let mut state = self.state.write().await;
                        state.page = tab_page;
                    }
                    HeaderRegion::PrevButton => {
                        return self.prev_track().await;
                    }
                    HeaderRegion::PlayButton => {
                        return self.toggle_pause().await;
                    }
                    HeaderRegion::PauseButton => {
                        return self.toggle_pause().await;
                    }
                    HeaderRegion::StopButton => {
                        return self.stop_playback().await;
                    }
                    HeaderRegion::NextButton => {
                        return self.next_track().await;
                    }
                }
            }
            return Ok(());
        }

        // Check now playing area (progress bar seeking)
        if y >= layout.now_playing.y && y < layout.now_playing.y + layout.now_playing.height {
            // The progress bar is on the last content line of the now_playing block.
            // The block has a 1-cell border, so inner area starts at y+1.
            // Progress bar row depends on layout height, but it's always the last inner row.
            let inner_bottom = layout.now_playing.y + layout.now_playing.height - 2; // -1 for border, -1 for 0-index
            if y == inner_bottom && duration > 0.0 {
                // Calculate seek position from x coordinate within the now_playing area
                // The progress bar renders centered: "MM:SS / MM:SS  [━━━━━────]"
                // We approximate: the bar occupies roughly the right portion of the inner area
                let inner_x_start = layout.now_playing.x + 1; // border
                let inner_width = layout.now_playing.width.saturating_sub(2);
                if inner_width > 15 && x >= inner_x_start {
                    let rel_x = x - inner_x_start;
                    // Time text is roughly "MM:SS / MM:SS  " = ~15 chars, bar fills the rest
                    let time_width = 15u16;
                    let bar_width = inner_width.saturating_sub(time_width + 2);
                    let bar_start = (inner_width.saturating_sub(time_width + 2 + bar_width)) / 2 + time_width + 2;
                    if bar_width > 0 && rel_x >= bar_start && rel_x < bar_start + bar_width {
                        let fraction = (rel_x - bar_start) as f64 / bar_width as f64;
                        let seek_pos = fraction * duration;
                        let _ = self.mpv.seek(seek_pos);
                        let mut state = self.state.write().await;
                        state.now_playing.position = seek_pos;
                    }
                }
            }
            return Ok(());
        }

        // Check content area
        if y >= layout.content.y && y < layout.content.y + layout.content.height {
            return self.handle_content_click(x, y, page, &layout).await;
        }

        Ok(())
    }

    /// Handle click within the content area
    async fn handle_content_click(
        &mut self,
        x: u16,
        y: u16,
        page: Page,
        layout: &LayoutAreas,
    ) -> Result<(), Error> {
        match page {
            Page::Artists => self.handle_artists_click(x, y, layout).await,
            Page::Queue => self.handle_queue_click(y, layout).await,
            Page::Playlists => self.handle_playlists_click(x, y, layout).await,
            _ => Ok(()),
        }
    }

    /// Handle click on artists page
    async fn handle_artists_click(
        &mut self,
        x: u16,
        y: u16,
        layout: &LayoutAreas,
    ) -> Result<(), Error> {
        use crate::ui::pages::artists::{build_tree_items, TreeItem};

        let mut state = self.state.write().await;
        let left = layout.content_left.unwrap_or(layout.content);
        let right = layout.content_right.unwrap_or(layout.content);

        if x >= left.x && x < left.x + left.width && y >= left.y && y < left.y + left.height {
            // Tree pane click — account for border (1 row top)
            let row_in_viewport = y.saturating_sub(left.y + 1) as usize;
            let item_index = state.artists.tree_scroll_offset + row_in_viewport;
            let tree_items = build_tree_items(&state);

            if item_index < tree_items.len() {
                let was_selected = state.artists.selected_index == Some(item_index);
                state.artists.focus = 0;
                state.artists.selected_index = Some(item_index);

                // Second click = activate (same as Enter)
                let is_second_click = was_selected
                    && self.last_click.map_or(false, |(lx, ly, t)| {
                        lx == x && ly == y && t.elapsed().as_millis() < 500
                    });

                if is_second_click {
                    // Activate: expand/collapse artist, or play album
                    match &tree_items[item_index] {
                        TreeItem::Artist { artist, expanded } => {
                            let artist_id = artist.id.clone();
                            let artist_name = artist.name.clone();
                            let was_expanded = *expanded;

                            if was_expanded {
                                state.artists.expanded.remove(&artist_id);
                            } else {
                                if !state.artists.albums_cache.contains_key(&artist_id) {
                                    drop(state);
                                    if let Some(ref client) = self.subsonic {
                                        match client.get_artist(&artist_id).await {
                                            Ok((_artist, albums)) => {
                                                let mut state = self.state.write().await;
                                                let count = albums.len();
                                                state.artists.albums_cache.insert(artist_id.clone(), albums);
                                                state.artists.expanded.insert(artist_id);
                                                info!("Loaded {} albums for {}", count, artist_name);
                                            }
                                            Err(e) => {
                                                let mut state = self.state.write().await;
                                                state.notify_error(format!("Failed to load: {}", e));
                                            }
                                        }
                                    }
                                    self.last_click = Some((x, y, std::time::Instant::now()));
                                    return Ok(());
                                } else {
                                    state.artists.expanded.insert(artist_id);
                                }
                            }
                        }
                        TreeItem::Album { album } => {
                            let album_id = album.id.clone();
                            let album_name = album.name.clone();
                            drop(state);

                            if let Some(ref client) = self.subsonic {
                                match client.get_album(&album_id).await {
                                    Ok((_album, songs)) => {
                                        if songs.is_empty() {
                                            let mut state = self.state.write().await;
                                            state.notify_error("Album has no songs");
                                            self.last_click = Some((x, y, std::time::Instant::now()));
                                            return Ok(());
                                        }

                                        let first_song = songs[0].clone();
                                        let stream_url = client.get_stream_url(&first_song.id);

                                        let mut state = self.state.write().await;
                                        let count = songs.len();
                                        state.queue.clear();
                                        state.queue.extend(songs.clone());
                                        state.queue_position = Some(0);
                                        state.artists.songs = songs;
                                        state.artists.selected_song = Some(0);
                                        state.artists.focus = 1;
                                        state.now_playing.song = Some(first_song.clone());
                                        state.now_playing.state = PlaybackState::Playing;
                                        state.now_playing.position = 0.0;
                                        state.now_playing.duration = first_song.duration.unwrap_or(0) as f64;
                                        state.now_playing.sample_rate = None;
                                        state.now_playing.bit_depth = None;
                                        state.now_playing.format = None;
                                        state.now_playing.channels = None;
                                        state.notify(format!("Playing album: {} ({} songs)", album_name, count));
                                        drop(state);

                                        if let Ok(url) = stream_url {
                                            if self.mpv.is_paused().unwrap_or(false) {
                                                let _ = self.mpv.resume();
                                            }
                                            if let Err(e) = self.mpv.loadfile(&url) {
                                                error!("Failed to play: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = self.state.write().await;
                                        state.notify_error(format!("Failed to load album: {}", e));
                                    }
                                }
                            }
                            self.last_click = Some((x, y, std::time::Instant::now()));
                            return Ok(());
                        }
                    }
                } else {
                    // First click on album: preview songs in right pane
                    if let TreeItem::Album { album } = &tree_items[item_index] {
                        let album_id = album.id.clone();
                        drop(state);
                        if let Some(ref client) = self.subsonic {
                            if let Ok((_album, songs)) = client.get_album(&album_id).await {
                                let mut state = self.state.write().await;
                                state.artists.songs = songs;
                                state.artists.selected_song = Some(0);
                            }
                        }
                        self.last_click = Some((x, y, std::time::Instant::now()));
                        return Ok(());
                    }
                }
            }
        } else if x >= right.x && x < right.x + right.width && y >= right.y && y < right.y + right.height {
            // Songs pane click
            let row_in_viewport = y.saturating_sub(right.y + 1) as usize;
            let item_index = state.artists.song_scroll_offset + row_in_viewport;

            if item_index < state.artists.songs.len() {
                let was_selected = state.artists.selected_song == Some(item_index);
                state.artists.focus = 1;
                state.artists.selected_song = Some(item_index);

                let is_second_click = was_selected
                    && self.last_click.map_or(false, |(lx, ly, t)| {
                        lx == x && ly == y && t.elapsed().as_millis() < 500
                    });

                if is_second_click {
                    // Play selected song
                    let song = state.artists.songs[item_index].clone();
                    let songs = state.artists.songs.clone();
                    state.queue.clear();
                    state.queue.extend(songs);
                    state.queue_position = Some(item_index);
                    state.now_playing.song = Some(song.clone());
                    state.now_playing.state = PlaybackState::Playing;
                    state.now_playing.position = 0.0;
                    state.now_playing.duration = song.duration.unwrap_or(0) as f64;
                    state.now_playing.sample_rate = None;
                    state.now_playing.bit_depth = None;
                    state.now_playing.format = None;
                    state.now_playing.channels = None;
                    state.notify(format!("Playing: {}", song.title));
                    drop(state);

                    if let Some(ref client) = self.subsonic {
                        if let Ok(url) = client.get_stream_url(&song.id) {
                            if self.mpv.is_paused().unwrap_or(false) {
                                let _ = self.mpv.resume();
                            }
                            if let Err(e) = self.mpv.loadfile(&url) {
                                error!("Failed to play: {}", e);
                            }
                        }
                    }
                    self.last_click = Some((x, y, std::time::Instant::now()));
                    return Ok(());
                }
            }
        }

        self.last_click = Some((x, y, std::time::Instant::now()));
        Ok(())
    }

    /// Handle click on queue page
    async fn handle_queue_click(&mut self, y: u16, layout: &LayoutAreas) -> Result<(), Error> {
        let mut state = self.state.write().await;
        let content = layout.content;

        // Account for border (1 row top)
        let row_in_viewport = y.saturating_sub(content.y + 1) as usize;
        let item_index = state.queue_state.scroll_offset + row_in_viewport;

        if item_index < state.queue.len() {
            let was_selected = state.queue_state.selected == Some(item_index);
            state.queue_state.selected = Some(item_index);

            let is_second_click = was_selected
                && self.last_click.map_or(false, |(_, ly, t)| {
                    ly == y && t.elapsed().as_millis() < 500
                });

            if is_second_click {
                drop(state);
                self.last_click = Some((0, y, std::time::Instant::now()));
                return self.play_queue_position(item_index).await;
            }
        }

        self.last_click = Some((0, y, std::time::Instant::now()));
        Ok(())
    }

    /// Handle click on playlists page
    async fn handle_playlists_click(
        &mut self,
        x: u16,
        y: u16,
        layout: &LayoutAreas,
    ) -> Result<(), Error> {
        let mut state = self.state.write().await;
        let left = layout.content_left.unwrap_or(layout.content);
        let right = layout.content_right.unwrap_or(layout.content);

        if x >= left.x && x < left.x + left.width && y >= left.y && y < left.y + left.height {
            // Playlists pane
            let row_in_viewport = y.saturating_sub(left.y + 1) as usize;
            let item_index = state.playlists.playlist_scroll_offset + row_in_viewport;

            if item_index < state.playlists.playlists.len() {
                let was_selected = state.playlists.selected_playlist == Some(item_index);
                state.playlists.focus = 0;
                state.playlists.selected_playlist = Some(item_index);

                let is_second_click = was_selected
                    && self.last_click.map_or(false, |(lx, ly, t)| {
                        lx == x && ly == y && t.elapsed().as_millis() < 500
                    });

                if is_second_click {
                    // Load playlist songs (same as Enter)
                    let playlist = state.playlists.playlists[item_index].clone();
                    let playlist_id = playlist.id.clone();
                    let playlist_name = playlist.name.clone();
                    drop(state);

                    if let Some(ref client) = self.subsonic {
                        match client.get_playlist(&playlist_id).await {
                            Ok((_playlist, songs)) => {
                                let mut state = self.state.write().await;
                                let count = songs.len();
                                state.playlists.songs = songs;
                                state.playlists.selected_song = if count > 0 { Some(0) } else { None };
                                state.playlists.focus = 1;
                                state.notify(format!("Loaded playlist: {} ({} songs)", playlist_name, count));
                            }
                            Err(e) => {
                                let mut state = self.state.write().await;
                                state.notify_error(format!("Failed to load playlist: {}", e));
                            }
                        }
                    }
                    self.last_click = Some((x, y, std::time::Instant::now()));
                    return Ok(());
                }
            }
        } else if x >= right.x && x < right.x + right.width && y >= right.y && y < right.y + right.height {
            // Songs pane
            let row_in_viewport = y.saturating_sub(right.y + 1) as usize;
            let item_index = state.playlists.song_scroll_offset + row_in_viewport;

            if item_index < state.playlists.songs.len() {
                let was_selected = state.playlists.selected_song == Some(item_index);
                state.playlists.focus = 1;
                state.playlists.selected_song = Some(item_index);

                let is_second_click = was_selected
                    && self.last_click.map_or(false, |(lx, ly, t)| {
                        lx == x && ly == y && t.elapsed().as_millis() < 500
                    });

                if is_second_click {
                    // Play selected song from playlist
                    let songs = state.playlists.songs.clone();
                    state.queue.clear();
                    state.queue.extend(songs);
                    drop(state);
                    self.last_click = Some((x, y, std::time::Instant::now()));
                    return self.play_queue_position(item_index).await;
                }
            }
        }

        self.last_click = Some((x, y, std::time::Instant::now()));
        Ok(())
    }

    /// Handle mouse scroll up (move selection up in current list)
    async fn handle_mouse_scroll_up(&mut self) -> Result<(), Error> {
        let mut state = self.state.write().await;
        match state.page {
            Page::Artists => {
                if state.artists.focus == 0 {
                    if let Some(sel) = state.artists.selected_index {
                        if sel > 0 {
                            state.artists.selected_index = Some(sel - 1);
                        }
                    }
                } else {
                    if let Some(sel) = state.artists.selected_song {
                        if sel > 0 {
                            state.artists.selected_song = Some(sel - 1);
                        }
                    }
                }
            }
            Page::Queue => {
                if let Some(sel) = state.queue_state.selected {
                    if sel > 0 {
                        state.queue_state.selected = Some(sel - 1);
                    }
                } else if !state.queue.is_empty() {
                    state.queue_state.selected = Some(0);
                }
            }
            Page::Playlists => {
                if state.playlists.focus == 0 {
                    if let Some(sel) = state.playlists.selected_playlist {
                        if sel > 0 {
                            state.playlists.selected_playlist = Some(sel - 1);
                        }
                    }
                } else {
                    if let Some(sel) = state.playlists.selected_song {
                        if sel > 0 {
                            state.playlists.selected_song = Some(sel - 1);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle mouse scroll down (move selection down in current list)
    async fn handle_mouse_scroll_down(&mut self) -> Result<(), Error> {
        let mut state = self.state.write().await;
        match state.page {
            Page::Artists => {
                if state.artists.focus == 0 {
                    let tree_items = crate::ui::pages::artists::build_tree_items(&state);
                    let max = tree_items.len().saturating_sub(1);
                    if let Some(sel) = state.artists.selected_index {
                        if sel < max {
                            state.artists.selected_index = Some(sel + 1);
                        }
                    } else if !tree_items.is_empty() {
                        state.artists.selected_index = Some(0);
                    }
                } else {
                    let max = state.artists.songs.len().saturating_sub(1);
                    if let Some(sel) = state.artists.selected_song {
                        if sel < max {
                            state.artists.selected_song = Some(sel + 1);
                        }
                    } else if !state.artists.songs.is_empty() {
                        state.artists.selected_song = Some(0);
                    }
                }
            }
            Page::Queue => {
                let max = state.queue.len().saturating_sub(1);
                if let Some(sel) = state.queue_state.selected {
                    if sel < max {
                        state.queue_state.selected = Some(sel + 1);
                    }
                } else if !state.queue.is_empty() {
                    state.queue_state.selected = Some(0);
                }
            }
            Page::Playlists => {
                if state.playlists.focus == 0 {
                    let max = state.playlists.playlists.len().saturating_sub(1);
                    if let Some(sel) = state.playlists.selected_playlist {
                        if sel < max {
                            state.playlists.selected_playlist = Some(sel + 1);
                        }
                    } else if !state.playlists.playlists.is_empty() {
                        state.playlists.selected_playlist = Some(0);
                    }
                } else {
                    let max = state.playlists.songs.len().saturating_sub(1);
                    if let Some(sel) = state.playlists.selected_song {
                        if sel < max {
                            state.playlists.selected_song = Some(sel + 1);
                        }
                    } else if !state.playlists.songs.is_empty() {
                        state.playlists.selected_song = Some(0);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle UI action from async tasks
    async fn handle_ui_action(&mut self, action: UiAction) {
match action {
    UiAction::UpdatePosition { position, duration } => {
        let mut state = self.state.write().await;
        state.now_playing.position = position;
        state.now_playing.duration = duration;
    }
    UiAction::UpdatePlaybackState(pbs) => {
        let mut state = self.state.write().await;
        state.now_playing.state = match pbs {
            PlaybackStateUpdate::Playing => PlaybackState::Playing,
            PlaybackStateUpdate::Paused => PlaybackState::Paused,
            PlaybackStateUpdate::Stopped => PlaybackState::Stopped,
        };
    }
    UiAction::UpdateAudioProperties {
        sample_rate,
        bit_depth,
        format,
    } => {
        let mut state = self.state.write().await;
        state.now_playing.sample_rate = sample_rate;
        state.now_playing.bit_depth = bit_depth;
        state.now_playing.format = format;
    }
    UiAction::TrackEnded => {
        // Advance to next track
        let mut state = self.state.write().await;
        if let Some(pos) = state.queue_position {
            if pos + 1 < state.queue.len() {
                state.queue_position = Some(pos + 1);
                // Would trigger play of next track
            } else {
                state.queue_position = None;
                state.now_playing.state = PlaybackState::Stopped;
            }
        }
    }
    UiAction::Notify { message, is_error } => {
        let mut state = self.state.write().await;
        if is_error {
            state.notify_error(message);
        } else {
            state.notify(message);
        }
    }
    UiAction::ArtistsLoaded(artists) => {
        let mut state = self.state.write().await;
        let has_artists = !artists.is_empty();
        state.artists.artists = artists;
        if has_artists && state.artists.selected_index.is_none() {
            state.artists.selected_index = Some(0);
        }
    }
    UiAction::AlbumsLoaded { artist_id, albums } => {
        let mut state = self.state.write().await;
        state.artists.albums_cache.insert(artist_id.clone(), albums);
        state.artists.expanded.insert(artist_id);
    }
    UiAction::SongsLoaded { songs, .. } => {
        let mut state = self.state.write().await;
        state.artists.songs = songs;
    }
    UiAction::PlaylistsLoaded(playlists) => {
        let mut state = self.state.write().await;
        state.playlists.playlists = playlists;
    }
    UiAction::PlaylistSongsLoaded { songs, .. } => {
        let mut state = self.state.write().await;
        state.playlists.songs = songs;
    }
    UiAction::ConnectionTestResult {
        success: _,
        message,
    } => {
        let mut state = self.state.write().await;
        state.server_state.status = Some(message);
    }
    UiAction::Redraw => {
        // Will redraw on next iteration
    }
}
    }

    /// Toggle play/pause
    async fn toggle_pause(&mut self) -> Result<(), Error> {
let state = self.state.read().await;
let is_playing = state.now_playing.state == PlaybackState::Playing;
let is_paused = state.now_playing.state == PlaybackState::Paused;
drop(state);

if !is_playing && !is_paused {
    return Ok(());
}

match self.mpv.toggle_pause() {
    Ok(now_paused) => {
        let mut state = self.state.write().await;
        if now_paused {
            state.now_playing.state = PlaybackState::Paused;
            debug!("Paused playback");
        } else {
            state.now_playing.state = PlaybackState::Playing;
            debug!("Resumed playback");
        }
    }
    Err(e) => {
        error!("Failed to toggle pause: {}", e);
    }
}
Ok(())
    }

    /// Pause playback (only if currently playing)
    async fn pause_playback(&mut self) -> Result<(), Error> {
let state = self.state.read().await;
if state.now_playing.state != PlaybackState::Playing {
    return Ok(());
}
drop(state);

match self.mpv.pause() {
    Ok(()) => {
        let mut state = self.state.write().await;
        state.now_playing.state = PlaybackState::Paused;
        debug!("Paused playback");
    }
    Err(e) => {
        error!("Failed to pause: {}", e);
    }
}
Ok(())
    }

    /// Resume playback (only if currently paused)
    async fn resume_playback(&mut self) -> Result<(), Error> {
let state = self.state.read().await;
if state.now_playing.state != PlaybackState::Paused {
    return Ok(());
}
drop(state);

match self.mpv.resume() {
    Ok(()) => {
        let mut state = self.state.write().await;
        state.now_playing.state = PlaybackState::Playing;
        debug!("Resumed playback");
    }
    Err(e) => {
        error!("Failed to resume: {}", e);
    }
}
Ok(())
    }

    /// Play next track in queue
    async fn next_track(&mut self) -> Result<(), Error> {
let state = self.state.read().await;
let queue_len = state.queue.len();
let current_pos = state.queue_position;
drop(state);

if queue_len == 0 {
    return Ok(());
}

let next_pos = match current_pos {
    Some(pos) if pos + 1 < queue_len => pos + 1,
    _ => {
        info!("Reached end of queue");
        let _ = self.mpv.stop();
        let mut state = self.state.write().await;
        state.now_playing.state = PlaybackState::Stopped;
        state.now_playing.position = 0.0;
        return Ok(());
    }
};

self.play_queue_position(next_pos).await
    }

    /// Play previous track in queue (or restart current if < 3 seconds in)
    async fn prev_track(&mut self) -> Result<(), Error> {
let state = self.state.read().await;
let queue_len = state.queue.len();
let current_pos = state.queue_position;
let position = state.now_playing.position;
drop(state);

if queue_len == 0 {
    return Ok(());
}

if position < 3.0 {
    if let Some(pos) = current_pos {
        if pos > 0 {
            return self.play_queue_position(pos - 1).await;
        }
    }
    if let Err(e) = self.mpv.seek(0.0) {
        error!("Failed to restart track: {}", e);
    } else {
        let mut state = self.state.write().await;
        state.now_playing.position = 0.0;
    }
    return Ok(());
}

debug!("Restarting current track (position: {:.1}s)", position);
if let Err(e) = self.mpv.seek(0.0) {
    error!("Failed to restart track: {}", e);
} else {
    let mut state = self.state.write().await;
    state.now_playing.position = 0.0;
}
Ok(())
    }

    /// Play a specific position in the queue
    async fn play_queue_position(&mut self, pos: usize) -> Result<(), Error> {
let state = self.state.read().await;
let song = match state.queue.get(pos) {
    Some(s) => s.clone(),
    None => return Ok(()),
};
drop(state);

let stream_url = if let Some(ref client) = self.subsonic {
    match client.get_stream_url(&song.id) {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to get stream URL: {}", e);
            let mut state = self.state.write().await;
            state.notify_error(format!("Failed to get stream URL: {}", e));
            return Ok(());
        }
    }
} else {
    return Ok(());
};

{
    let mut state = self.state.write().await;
    state.queue_position = Some(pos);
    state.now_playing.song = Some(song.clone());
    state.now_playing.state = PlaybackState::Playing;
    state.now_playing.position = 0.0;
    state.now_playing.duration = song.duration.unwrap_or(0) as f64;
    state.now_playing.sample_rate = None;
    state.now_playing.bit_depth = None;
    state.now_playing.format = None;
    state.now_playing.channels = None;
}

info!("Playing: {} (queue pos {})", song.title, pos);
if self.mpv.is_paused().unwrap_or(false) {
    let _ = self.mpv.resume();
}
if let Err(e) = self.mpv.loadfile(&stream_url) {
    error!("Failed to play: {}", e);
    let mut state = self.state.write().await;
    state.notify_error(format!("MPV error: {}", e));
    return Ok(());
}

self.preload_next_track(pos).await;

Ok(())
    }

    /// Pre-load the next track into MPV's playlist for gapless playback
    async fn preload_next_track(&mut self, current_pos: usize) {
let state = self.state.read().await;
let next_pos = current_pos + 1;

if next_pos >= state.queue.len() {
    return;
}

let next_song = match state.queue.get(next_pos) {
    Some(s) => s.clone(),
    None => return,
};
drop(state);

if let Some(ref client) = self.subsonic {
    if let Ok(url) = client.get_stream_url(&next_song.id) {
        debug!("Pre-loading next track for gapless: {}", next_song.title);
        if let Err(e) = self.mpv.loadfile_append(&url) {
            debug!("Failed to pre-load next track: {}", e);
        } else if let Ok(count) = self.mpv.get_playlist_count() {
            if count < 2 {
                warn!("Preload may have failed: playlist count is {} (expected 2)", count);
            } else {
                debug!("Preload confirmed: playlist count is {}", count);
            }
        }
    }
}
    }

    /// Stop playback and clear the queue
    async fn stop_playback(&mut self) -> Result<(), Error> {
if let Err(e) = self.mpv.stop() {
    error!("Failed to stop: {}", e);
}

let mut state = self.state.write().await;
state.now_playing.state = PlaybackState::Stopped;
state.now_playing.song = None;
state.now_playing.position = 0.0;
state.now_playing.duration = 0.0;
state.now_playing.sample_rate = None;
state.now_playing.bit_depth = None;
state.now_playing.format = None;
state.now_playing.channels = None;
state.queue.clear();
state.queue_position = None;
Ok(())
    }
}

/// Convert vt100 color to our CavaColor type
fn vt100_color_to_cava(color: vt100::Color) -> CavaColor {
    match color {
        vt100::Color::Default => CavaColor::Default,
        vt100::Color::Idx(i) => CavaColor::Indexed(i),
        vt100::Color::Rgb(r, g, b) => CavaColor::Rgb(r, g, b),
    }
}

/// Generate a cava configuration string with theme-appropriate gradient colors
fn generate_cava_config(g: &[String; 8], h: &[String; 8]) -> String {

    format!(
        "\
[general]
framerate = 60
autosens = 1
overshoot = 0
bars = 0
bar_width = 1
bar_spacing = 0
lower_cutoff_freq = 10
higher_cutoff_freq = 18000

[input]
sample_rate = 96000
sample_bits = 32
remix = 1

[output]
method = noncurses
orientation = horizontal
channels = mono
mono_option = average
synchronized_sync = 1
disable_blanking = 1

[color]
gradient = 1
gradient_color_1 = '{g0}'
gradient_color_2 = '{g1}'
gradient_color_3 = '{g2}'
gradient_color_4 = '{g3}'
gradient_color_5 = '{g4}'
gradient_color_6 = '{g5}'
gradient_color_7 = '{g6}'
gradient_color_8 = '{g7}'
horizontal_gradient = 1
horizontal_gradient_color_1 = '{h0}'
horizontal_gradient_color_2 = '{h1}'
horizontal_gradient_color_3 = '{h2}'
horizontal_gradient_color_4 = '{h3}'
horizontal_gradient_color_5 = '{h4}'
horizontal_gradient_color_6 = '{h5}'
horizontal_gradient_color_7 = '{h6}'
horizontal_gradient_color_8 = '{h7}'

[smoothing]
monstercat = 0
waves = 0
noise_reduction = 11
",
        g0 = g[0], g1 = g[1], g2 = g[2], g3 = g[3],
        g4 = g[4], g5 = g[5], g6 = g[6], g7 = g[7],
        h0 = h[0], h1 = h[1], h2 = h[2], h3 = h[3],
        h4 = h[4], h5 = h[5], h6 = h[6], h7 = h[7],
    )
}
