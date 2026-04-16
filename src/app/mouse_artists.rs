use tracing::error;

use crate::error::Error;

use super::*;

impl App {
    /// Handle click on artists page
    pub(super) async fn handle_artists_click(
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
                    && self.last_click.is_some_and(|(lx, ly, t)| {
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
                            } else if !state.artists.albums_cache.contains_key(&artist_id) {
                                drop(state);
                                if let Some(ref client) = self.subsonic {
                                    match client.get_artist(&artist_id).await {
                                        Ok((_artist, albums)) => {
                                            let mut state = self.state.write().await;
                                            let count = albums.len();
                                            state
                                                .artists
                                                .albums_cache
                                                .insert(artist_id.clone(), albums);
                                            state.artists.expanded.insert(artist_id);
                                            tracing::info!(
                                                "Loaded {} albums for {}",
                                                count,
                                                artist_name
                                            );
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
                                            self.last_click =
                                                Some((x, y, std::time::Instant::now()));
                                            return Ok(());
                                        }

                                        let mut state = self.state.write().await;
                                        let count = songs.len();
                                        state.queue.clear();
                                        state.queue.extend(songs.clone());
                                        state.artists.songs = songs;
                                        state.artists.selected_song = Some(0);
                                        state.artists.focus = 1;

                                        state.notify(format!(
                                            "Playing album: {} ({} songs)",
                                            album_name, count
                                        ));

                                        drop(state);

                                        return self.play_queue_position(0).await;
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
        } else if x >= right.x
            && x < right.x + right.width
            && y >= right.y
            && y < right.y + right.height
        {
            // Songs pane click
            let row_in_viewport = y.saturating_sub(right.y + 1) as usize;
            let item_index = state.artists.song_scroll_offset + row_in_viewport;

            if item_index < state.artists.songs.len() {
                let was_selected = state.artists.selected_song == Some(item_index);
                state.artists.focus = 1;
                state.artists.selected_song = Some(item_index);

                let is_second_click = was_selected
                    && self.last_click.is_some_and(|(lx, ly, t)| {
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
}
