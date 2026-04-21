use super::*;

impl App {
    /// Page size used by the All-songs view.
    const ALL_SONGS_PAGE_SIZE: usize = 50;

    /// Page size used by the Albums view (max allowed by getAlbumList2).
    const ALL_ALBUMS_PAGE_SIZE: usize = 500;

    /// Fetch the next page of songs for the "All" view via `search3`.
    ///
    /// When `append` is `false` the song list is replaced (used on first load
    /// or after the filter changes).  When `append` is `true` the new songs are
    /// appended to the existing list (used for infinite-scroll pagination).
    pub async fn get_all_songs(&mut self, append: bool) {
        if let Some(ref client) = self.subsonic {
            let (offset, filter) = {
                let state = self.state.read().await;
                (state.browse.all_songs_offset, state.browse.filter.clone())
            };

            {
                let mut state = self.state.write().await;
                state.browse.all_songs_loading = true;
            }

            match client
                .search_songs(&filter, offset, Self::ALL_SONGS_PAGE_SIZE)
                .await
            {
                Ok(songs) => {
                    let mut state = self.state.write().await;
                    let fetched = songs.len();
                    let has_more = fetched == Self::ALL_SONGS_PAGE_SIZE;

                    if append {
                        state.browse.songs.extend(songs);
                    } else {
                        state.browse.songs = songs;
                        state.browse.selected_index = if fetched > 0 { Some(0) } else { None };
                        state.browse.scroll_offset = 0;
                    }

                    state.browse.all_songs_offset = offset + fetched;
                    state.browse.all_songs_has_more = has_more;
                    state.browse.all_songs_loading = false;
                }
                Err(e) => {
                    error!("Failed to load all songs: {}", e);
                    let mut state = self.state.write().await;
                    state.browse.all_songs_loading = false;
                    state.notify_error(format!("Failed to load all songs: {}", e));
                }
            }
        }
    }

    pub async fn get_starred_songs(&mut self) {
        if let Some(ref client) = self.subsonic {
            match client.get_starred_songs().await {
                Ok(songs) => {
                    let mut state = self.state.write().await;
                    state.browse.backing_songs = songs;
                    state.browse.apply_filter();
                }
                Err(e) => {
                    error!("Failed to load starred songs: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to load starred songs: {}", e));
                }
            }
        }
    }

    pub async fn get_random_songs(&mut self) {
        if let Some(ref client) = self.subsonic {
            let random_songs_count = self.state.read().await.config.random_songs_count;

            match client.get_random_songs(random_songs_count).await {
                Ok(songs) => {
                    let mut state = self.state.write().await;
                    state.browse.backing_songs = songs;
                    state.browse.apply_filter();
                }
                Err(e) => {
                    error!("Failed to load random songs: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to load random songs: {}", e));
                }
            }
        }
    }

    /// Fetch the next page of albums for the Albums view via `getAlbumList2`.
    ///
    /// When `append` is `false` the album list is replaced (used on first load or after
    /// filter changes). When `append` is `true` new albums are appended (infinite scroll).
    pub async fn get_browse_albums(&mut self, append: bool) {
        if let Some(ref client) = self.subsonic {
            let offset = {
                let state = self.state.read().await;
                state.browse.albums_offset
            };

            {
                let mut state = self.state.write().await;
                state.browse.albums_loading = true;
            }

            match client
                .get_album_list("alphabeticalByName", Self::ALL_ALBUMS_PAGE_SIZE, offset)
                .await
            {
                Ok(albums) => {
                    let mut state = self.state.write().await;
                    let fetched = albums.len();
                    let has_more = fetched == Self::ALL_ALBUMS_PAGE_SIZE;

                    let sel = state.browse.selected_album;
                    let scroll = state.browse.album_scroll_offset;
                    if append {
                        state.browse.backing_albums.extend(albums);
                    } else {
                        state.browse.backing_albums = albums;
                    }
                    state.browse.apply_album_filter();
                    if append {
                        state.browse.selected_album = sel;
                        state.browse.album_scroll_offset = scroll;
                    }

                    state.browse.albums_offset = offset + fetched;
                    state.browse.albums_has_more = has_more;
                    state.browse.albums_loading = false;
                }
                Err(e) => {
                    error!("Failed to load albums: {}", e);
                    let mut state = self.state.write().await;
                    state.browse.albums_loading = false;
                    state.notify_error(format!("Failed to load albums: {}", e));
                }
            }
        }
    }

    pub async fn get_starred_albums(&mut self) {
        if let Some(ref client) = self.subsonic {
            match client.get_starred_albums().await {
                Ok(albums) => {
                    let mut state = self.state.write().await;
                    state.browse.backing_albums = albums;
                    state.browse.apply_album_filter();
                }
                Err(e) => {
                    error!("Failed to load starred albums: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to load starred albums: {}", e));
                }
            }
        }
    }

    pub async fn get_random_albums(&mut self) {
        if let Some(ref client) = self.subsonic {
            let random_songs_count = self.state.read().await.config.random_songs_count;
            match client.get_random_albums(random_songs_count).await {
                Ok(albums) => {
                    let mut state = self.state.write().await;
                    state.browse.backing_albums = albums;
                    state.browse.apply_album_filter();
                }
                Err(e) => {
                    error!("Failed to load random albums: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to load random albums: {}", e));
                }
            }
        }
    }

    pub async fn get_artists(&mut self) {
        if let Some(ref client) = self.subsonic {
            match client.get_artists().await {
                Ok(artists) => {
                    let mut state = self.state.write().await;
                    let count = artists.len();
                    state.artists.artists = artists;
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
        }
    }

    pub async fn get_playlists(&mut self) {
        if let Some(ref client) = self.subsonic {
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

    pub async fn unstar_song(&mut self, id: String) {
        if let Some(ref client) = self.subsonic {
            match client.unstar_song(&id).await {
                Ok(_) => {
                    let mut state = self.state.write().await;
                    state.notify("Song has been un-starred");
                    info!("Song un-starred");
                }
                Err(e) => {
                    error!("Failed to un-star song: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to un-star song: {}", e));
                }
            }
        }
    }

    pub async fn star_song(&mut self, id: String) {
        if let Some(ref client) = self.subsonic {
            match client.star_song(&id).await {
                Ok(_) => {
                    let mut state = self.state.write().await;
                    state.notify("Song has been starred");
                    info!("Song starred");
                }
                Err(e) => {
                    error!("Failed to star song: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to star song: {}", e));
                }
            }
        }
    }

    pub async fn unstar_artist(&mut self, id: String) {
        if let Some(ref client) = self.subsonic {
            match client.unstar_artist(&id).await {
                Ok(_) => {
                    let mut state = self.state.write().await;
                    state.notify("Artist has been un-starred");
                    info!("Artist un-starred");
                }
                Err(e) => {
                    error!("Failed to un-star artist: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to un-star artist: {}", e));
                }
            }
        }
    }

    pub async fn star_artist(&mut self, id: String) {
        if let Some(ref client) = self.subsonic {
            match client.star_artist(&id).await {
                Ok(_) => {
                    let mut state = self.state.write().await;
                    state.notify("Artist has been starred");
                    info!("Artist starred");
                }
                Err(e) => {
                    error!("Failed to star artist: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to star artist: {}", e));
                }
            }
        }
    }

    pub async fn unstar_album(&mut self, id: String) {
        if let Some(ref client) = self.subsonic {
            match client.unstar_album(&id).await {
                Ok(_) => {
                    let mut state = self.state.write().await;
                    state.notify("Album has been un-starred");
                    info!("Album un-starred");
                }
                Err(e) => {
                    error!("Failed to un-star album: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to un-star album: {}", e));
                }
            }
        }
    }

    pub async fn star_album(&mut self, id: String) {
        if let Some(ref client) = self.subsonic {
            match client.star_album(&id).await {
                Ok(_) => {
                    let mut state = self.state.write().await;
                    state.notify("Album has been starred");
                    info!("Album starred");
                }
                Err(e) => {
                    error!("Failed to star album: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to star album: {}", e));
                }
            }
        }
    }
}
