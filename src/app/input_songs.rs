use crossterm::event::{self, KeyCode};

use crate::app::models::SongOption;
use crate::error::Error;

use super::*;

impl App {
    pub(super) async fn handle_songs_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        // Filter input mode
        {
            let filter_active = self.state.read().await.songs.filter_active;
            if filter_active {
                return self.handle_songs_filter_key(key).await;
            }
        }

        let mut state = self.state.write().await;

        match key.code {
            // Activate filter
            KeyCode::Char('/') => {
                state.songs.filter_active = true;
            }

            // Navigation: options pane (focus == 0)
            KeyCode::Up | KeyCode::Char('k') if state.songs.focus == 0 => {
                let next = match state.songs.selected_option {
                    Some(SongOption::All) => None,
                    Some(SongOption::Starred) => Some(SongOption::All),
                    Some(SongOption::Random) => Some(SongOption::Starred),
                    None => None,
                };
                if let Some(opt) = next {
                    state.songs.selected_option = Some(opt.clone());
                    state.songs.scroll_offset = 0;
                    drop(state);
                    self.load_song_option(opt).await;
                }
            }

            KeyCode::Down | KeyCode::Char('j') if state.songs.focus == 0 => {
                let next = match state.songs.selected_option {
                    Some(SongOption::All) => Some(SongOption::Starred),
                    Some(SongOption::Starred) => Some(SongOption::Random),
                    Some(SongOption::Random) => None,
                    None => None,
                };
                if let Some(opt) = next {
                    state.songs.selected_option = Some(opt.clone());
                    state.songs.scroll_offset = 0;
                    drop(state);
                    self.load_song_option(opt).await;
                }
            }

            // Navigation: song list (focus == 1)
            KeyCode::Up | KeyCode::Char('k') if state.songs.focus == 1 => {
                if let Some(sel) = state.songs.selected_index {
                    if sel > 0 {
                        state.songs.selected_index = Some(sel - 1);
                    }
                } else if !state.songs.songs.is_empty() {
                    state.songs.selected_index = Some(0);
                }
            }

            KeyCode::Down | KeyCode::Char('j') if state.songs.focus == 1 => {
                let len = state.songs.songs.len();
                let max = len.saturating_sub(1);
                if let Some(sel) = state.songs.selected_index {
                    if sel < max {
                        state.songs.selected_index = Some(sel + 1);
                    }
                } else if len > 0 {
                    state.songs.selected_index = Some(0);
                }

                // Trigger infinite-scroll load for the All option
                let should_load_more = state.songs.selected_option == Some(SongOption::All)
                    && state.songs.all_songs_has_more
                    && !state.songs.all_songs_loading
                    && state
                        .songs
                        .selected_index
                        .map(|i| i + INFINITE_SCROLL_LOOKAHEAD >= state.songs.songs.len())
                        .unwrap_or(false);

                drop(state);

                if should_load_more {
                    self.get_all_songs(true).await;
                }
                return Ok(());
            }

            // Play selected song
            KeyCode::Enter => {
                let selected_song = state
                    .songs
                    .selected_index
                    .filter(|&idx| idx < state.songs.songs.len());

                let Some(selected_song) = selected_song else {
                    return Ok(());
                };

                state.queue.clear();
                let songs = state.songs.songs.clone();
                state.queue.extend(songs);

                drop(state);

                return self.play_queue_position(selected_song).await;
            }

            // Tab switches focus between panes
            KeyCode::Tab => state.songs.focus = if state.songs.focus == 1 { 0 } else { 1 },

            // Star / un-star
            KeyCode::Char('f') if state.songs.focus == 1 => {
                let selected_song_idx = state
                    .songs
                    .selected_index
                    .filter(|&idx| idx < state.songs.songs.len());

                let Some(selected_song_idx) = selected_song_idx else {
                    return Ok(());
                };

                let song = &mut state.songs.songs[selected_song_idx];
                let id = song.id.clone();
                let was_starred = song.starred.is_some();

                if song.starred.is_some() {
                    song.starred = None;
                } else {
                    song.starred = Some("starred".to_string());
                }

                let refresh_needed = state.songs.selected_option == Some(SongOption::Starred);
                drop(state);

                if was_starred {
                    self.unstar_song(id).await;
                } else {
                    self.star_song(id).await;
                }

                if refresh_needed {
                    self.get_starred_songs().await;
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Handle a key event while filter input is active.
    async fn handle_songs_filter_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        match key.code {
            KeyCode::Esc => {
                let option = {
                    let mut state = self.state.write().await;
                    state.songs.filter_active = false;
                    state.songs.filter.clear();
                    state.songs.selected_option.clone()
                };
                self.songs_filter_debounce = None;
                self.reload_after_filter_change(option).await;
            }
            KeyCode::Enter => {
                let option = {
                    let mut state = self.state.write().await;
                    state.songs.filter_active = false;
                    state.songs.focus = 1;
                    state.songs.selected_option.clone()
                };
                // Commit the search immediately on Enter, cancelling any pending debounce.
                self.songs_filter_debounce = None;
                self.reload_after_filter_change(option).await;
            }
            KeyCode::Backspace => {
                let option = {
                    let mut state = self.state.write().await;
                    state.songs.filter.pop();
                    state.songs.selected_option.clone()
                };
                if option == Some(SongOption::All) {
                    // Debounce — fire the search 300 ms after typing stops.
                    self.songs_filter_debounce = Some(std::time::Instant::now());
                } else {
                    self.state.write().await.songs.apply_filter();
                }
            }
            KeyCode::Char(c) => {
                let option = {
                    let mut state = self.state.write().await;
                    state.songs.filter.push(c);
                    state.songs.selected_option.clone()
                };
                if option == Some(SongOption::All) {
                    self.songs_filter_debounce = Some(std::time::Instant::now());
                } else {
                    self.state.write().await.songs.apply_filter();
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// After the filter text changes, reload data for the current option.
    ///
    /// * `All` – reset pagination and fetch the first page from the server.
    /// * `Starred` / `Random` – filter the already-loaded `backing_songs`
    ///   in-place (no network call needed).
    async fn reload_after_filter_change(&mut self, option: Option<SongOption>) {
        match option {
            Some(SongOption::All) => {
                {
                    let mut state = self.state.write().await;
                    state.songs.all_songs_offset = 0;
                    state.songs.all_songs_has_more = true;
                }
                self.get_all_songs(false).await;
            }
            _ => {
                self.state.write().await.songs.apply_filter();
            }
        }
    }

    /// Load songs for the given option (called when the user switches options).
    async fn load_song_option(&mut self, opt: SongOption) {
        match opt {
            SongOption::All => {
                {
                    let mut state = self.state.write().await;
                    state.songs.all_songs_offset = 0;
                    state.songs.all_songs_has_more = true;
                }
                self.get_all_songs(false).await;
            }
            SongOption::Starred => self.get_starred_songs().await,
            SongOption::Random => self.get_random_songs().await,
        }
    }
}
