use crossterm::event::{self, KeyCode};

use crate::error::Error;

use super::*;

impl App {
    pub(super) async fn handle_browse_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        // Filter input mode
        {
            let filter_active = self.state.read().await.browse.filter_active;
            if filter_active {
                return self.handle_browse_filter_key(key).await;
            }
        }

        let mut state = self.state.write().await;

        match key.code {
            // Left/Right: switch between Songs and Albums
            KeyCode::Left => {
                if state.browse.browse_tab == BrowseTab::Albums {
                    state.browse.browse_tab = BrowseTab::Songs;
                    state.browse.filter.clear();
                    state.browse.focus = 0;

                    let selected_option = state.browse.selected_option.clone();
                    drop(state);
                    self.songs_filter_debounce = None;

                    self.load_song_option(selected_option.unwrap_or(SongOption::All))
                        .await;
                }
            }
            KeyCode::Right => {
                if state.browse.browse_tab == BrowseTab::Songs {
                    state.browse.browse_tab = BrowseTab::Albums;
                    state.browse.filter.clear();
                    state.browse.focus = 0;

                    let selected_option = state.browse.selected_option.clone();
                    drop(state);
                    self.songs_filter_debounce = None;

                    self.load_album_option(selected_option.unwrap_or(SongOption::All))
                        .await;
                }
            }

            // Activate filter
            KeyCode::Char('/') => {
                state.browse.filter_active = true;
            }

            // Tab: cycle focus 0 ↔ 1 (same for both modes)
            KeyCode::Tab => {
                state.browse.focus = if state.browse.focus == 0 { 1 } else { 0 };
            }

            // Focus 0: Options pane (shared by both tabs)
            KeyCode::Up | KeyCode::Char('k') if state.browse.focus == 0 => {
                let next = match state.browse.selected_option {
                    Some(SongOption::All) => None,
                    Some(SongOption::Starred) => Some(SongOption::All),
                    Some(SongOption::Random) => Some(SongOption::Starred),
                    None => None,
                };

                if let Some(opt) = next {
                    state.browse.selected_option = Some(opt.clone());
                    state.browse.scroll_offset = 0;
                    state.browse.album_scroll_offset = 0;
                    let tab = state.browse.browse_tab.clone();
                    drop(state);
                    match tab {
                        BrowseTab::Songs => self.load_song_option(opt).await,
                        BrowseTab::Albums => self.load_album_option(opt).await,
                    }
                }
            }

            KeyCode::Down | KeyCode::Char('j') if state.browse.focus == 0 => {
                let next = match state.browse.selected_option {
                    Some(SongOption::All) => Some(SongOption::Starred),
                    Some(SongOption::Starred) => Some(SongOption::Random),
                    Some(SongOption::Random) => None,
                    None => None,
                };

                if let Some(opt) = next {
                    state.browse.selected_option = Some(opt.clone());
                    state.browse.scroll_offset = 0;
                    state.browse.album_scroll_offset = 0;
                    let tab = state.browse.browse_tab.clone();
                    drop(state);
                    match tab {
                        BrowseTab::Songs => self.load_song_option(opt).await,
                        BrowseTab::Albums => self.load_album_option(opt).await,
                    }
                }
            }

            // Focus 1: Songs list
            KeyCode::Up | KeyCode::Char('k')
                if state.browse.focus == 1 && state.browse.browse_tab == BrowseTab::Songs =>
            {
                if let Some(sel) = state.browse.selected_index {
                    if sel > 0 {
                        state.browse.selected_index = Some(sel - 1);
                    }
                } else if !state.browse.songs.is_empty() {
                    state.browse.selected_index = Some(0);
                }
            }

            KeyCode::Down | KeyCode::Char('j')
                if state.browse.focus == 1 && state.browse.browse_tab == BrowseTab::Songs =>
            {
                let len = state.browse.songs.len();
                let max = len.saturating_sub(1);
                if let Some(sel) = state.browse.selected_index {
                    if sel < max {
                        state.browse.selected_index = Some(sel + 1);
                    }
                } else if len > 0 {
                    state.browse.selected_index = Some(0);
                }

                let should_load_more = state.browse.selected_option == Some(SongOption::All)
                    && state.browse.all_songs_has_more
                    && !state.browse.all_songs_loading
                    && state
                        .browse
                        .selected_index
                        .map(|i| i + INFINITE_SCROLL_LOOKAHEAD >= state.browse.songs.len())
                        .unwrap_or(false);

                drop(state);

                if should_load_more {
                    self.get_all_songs(true).await;
                }
                return Ok(());
            }

            // Focus 1: Albums list
            KeyCode::Up | KeyCode::Char('k')
                if state.browse.focus == 1 && state.browse.browse_tab == BrowseTab::Albums =>
            {
                if let Some(sel) = state.browse.selected_album {
                    if sel > 0 {
                        state.browse.selected_album = Some(sel - 1);
                    }
                } else if !state.browse.albums.is_empty() {
                    state.browse.selected_album = Some(0);
                }
            }

            KeyCode::Down | KeyCode::Char('j')
                if state.browse.focus == 1 && state.browse.browse_tab == BrowseTab::Albums =>
            {
                let len = state.browse.albums.len();
                let max = len.saturating_sub(1);
                if let Some(sel) = state.browse.selected_album {
                    if sel < max {
                        state.browse.selected_album = Some(sel + 1);
                    }
                } else if len > 0 {
                    state.browse.selected_album = Some(0);
                }

                let should_load_more = state.browse.selected_option == Some(SongOption::All)
                    && state.browse.albums_has_more
                    && !state.browse.albums_loading
                    && state
                        .browse
                        .selected_album
                        .map(|i| i + INFINITE_SCROLL_LOOKAHEAD >= state.browse.albums.len())
                        .unwrap_or(false);

                drop(state);

                if should_load_more {
                    self.get_browse_albums(true).await;
                }
                return Ok(());
            }

            // Enter
            KeyCode::Enter if state.browse.browse_tab == BrowseTab::Albums => {
                let selected = state
                    .browse
                    .selected_album
                    .filter(|&idx| idx < state.browse.albums.len());

                let Some(album_idx) = selected else {
                    return Ok(());
                };

                let album_id = state.browse.albums[album_idx].id.clone();
                let album_name = state.browse.albums[album_idx].name.clone();
                drop(state);

                if let Some(ref client) = self.subsonic {
                    match client.get_album(&album_id).await {
                        Ok((_album, songs)) => {
                            let count = songs.len();
                            let mut state = self.state.write().await;
                            state.queue.clear();
                            state.queue.extend(songs);
                            state.notify(format!("Playing: {} ({} songs)", album_name, count));
                            drop(state);
                            return self.play_queue_position(0).await;
                        }
                        Err(e) => {
                            self.state
                                .write()
                                .await
                                .notify_error(format!("Failed to load album: {}", e));
                        }
                    }
                }
                return Ok(());
            }

            KeyCode::Enter => {
                let selected_song = state
                    .browse
                    .selected_index
                    .filter(|&idx| idx < state.browse.songs.len());

                let Some(selected_song) = selected_song else {
                    return Ok(());
                };

                state.queue.clear();
                let songs = state.browse.songs.clone();
                state.queue.extend(songs);
                drop(state);

                return self.play_queue_position(selected_song).await;
            }

            // f: star / un-star
            KeyCode::Char('f')
                if state.browse.focus == 1 && state.browse.browse_tab == BrowseTab::Albums =>
            {
                let selected = state
                    .browse
                    .selected_album
                    .filter(|&idx| idx < state.browse.albums.len());

                let Some(album_idx) = selected else {
                    return Ok(());
                };

                let album = &mut state.browse.albums[album_idx];
                let id = album.id.clone();
                let was_starred = album.starred.is_some();
                let new_starred = if was_starred {
                    None
                } else {
                    Some("starred".to_string())
                };
                album.starred = new_starred.clone();

                if let Some(backing) = state.browse.backing_albums.iter_mut().find(|a| a.id == id) {
                    backing.starred = new_starred;
                }

                let refresh_needed = state.browse.selected_option == Some(SongOption::Starred);
                drop(state);

                if was_starred {
                    self.unstar_album(id).await;
                } else {
                    self.star_album(id).await;
                }

                if refresh_needed {
                    self.get_starred_albums().await;
                }
            }

            KeyCode::Char('f') if state.browse.focus == 1 => {
                let selected_song_idx = state
                    .browse
                    .selected_index
                    .filter(|&idx| idx < state.browse.songs.len());

                let Some(selected_song_idx) = selected_song_idx else {
                    return Ok(());
                };

                let song = &mut state.browse.songs[selected_song_idx];
                let id = song.id.clone();
                let was_starred = song.starred.is_some();
                let new_starred = if was_starred {
                    None
                } else {
                    Some("starred".to_string())
                };
                song.starred = new_starred.clone();

                if let Some(backing) =
                    state.browse.backing_songs.iter_mut().find(|s| s.id == id)
                {
                    backing.starred = new_starred;
                }

                let refresh_needed = state.browse.selected_option == Some(SongOption::Starred);
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
    async fn handle_browse_filter_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        match key.code {
            KeyCode::Esc => {
                {
                    let mut state = self.state.write().await;
                    state.browse.filter_active = false;
                    state.browse.filter.clear();
                }
                self.songs_filter_debounce = None;
                self.reload_after_filter_change().await;
            }
            KeyCode::Enter => {
                {
                    let mut state = self.state.write().await;
                    state.browse.filter_active = false;
                    state.browse.focus = 1;
                }
                self.songs_filter_debounce = None;
                self.reload_after_filter_change().await;
            }
            KeyCode::Backspace => {
                let (tab, option) = {
                    let mut state = self.state.write().await;
                    state.browse.filter.pop();
                    (
                        state.browse.browse_tab.clone(),
                        state.browse.selected_option.clone(),
                    )
                };
                self.apply_filter_for(tab, option).await;
            }
            KeyCode::Char(c) => {
                let (tab, option) = {
                    let mut state = self.state.write().await;
                    state.browse.filter.push(c);
                    (
                        state.browse.browse_tab.clone(),
                        state.browse.selected_option.clone(),
                    )
                };
                self.apply_filter_for(tab, option).await;
            }
            _ => {}
        }
        Ok(())
    }

    async fn apply_filter_for(&mut self, tab: BrowseTab, option: Option<SongOption>) {
        match tab {
            BrowseTab::Albums => {
                self.state.write().await.browse.apply_album_filter();
            }
            BrowseTab::Songs => {
                if option == Some(SongOption::All) {
                    self.songs_filter_debounce = Some(std::time::Instant::now());
                } else {
                    self.state.write().await.browse.apply_filter();
                }
            }
        }
    }

    /// After Esc/Enter clears filter, reload data for the current tab/option.
    async fn reload_after_filter_change(&mut self) {
        let (tab, option) = {
            let state = self.state.read().await;
            (
                state.browse.browse_tab.clone(),
                state.browse.selected_option.clone(),
            )
        };

        match tab {
            BrowseTab::Albums => {
                self.state.write().await.browse.apply_album_filter();
            }
            BrowseTab::Songs => match option {
                Some(SongOption::All) => {
                    {
                        let mut state = self.state.write().await;
                        state.browse.all_songs_offset = 0;
                        state.browse.all_songs_has_more = true;
                    }
                    self.get_all_songs(false).await;
                }
                _ => {
                    self.state.write().await.browse.apply_filter();
                }
            },
        }
    }

    async fn load_song_option(&mut self, opt: SongOption) {
        match opt {
            SongOption::All => {
                {
                    let mut state = self.state.write().await;
                    state.browse.all_songs_offset = 0;
                    state.browse.all_songs_has_more = true;
                }
                self.get_all_songs(false).await;
            }
            SongOption::Starred => self.get_starred_songs().await,
            SongOption::Random => self.get_random_songs().await,
        }
    }

    async fn load_album_option(&mut self, opt: SongOption) {
        match opt {
            SongOption::All => {
                {
                    let mut state = self.state.write().await;
                    state.browse.albums_offset = 0;
                    state.browse.albums_has_more = true;
                }
                self.get_browse_albums(false).await;
            }
            SongOption::Starred => self.get_starred_albums().await,
            SongOption::Random => self.get_random_albums().await,
        }
    }
}
