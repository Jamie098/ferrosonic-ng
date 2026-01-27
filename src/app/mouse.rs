use crossterm::event::{self, MouseButton, MouseEventKind};
use tracing::error;

use crate::error::Error;

use super::*;

impl App {
    /// Handle mouse input
    pub(super) async fn handle_mouse(&mut self, mouse: event::MouseEvent) -> Result<(), Error> {
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
                                                tracing::info!("Loaded {} albums for {}", count, artist_name);
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
}
