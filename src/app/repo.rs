use super::*;

impl App {
    pub async fn get_starred_songs(&mut self) {
        if let Some(ref client) = self.subsonic {
            match client.get_starred_songs().await {
                Ok(songs) => {
                    let mut state = self.state.write().await;
                    let count = songs.len();
                    state.songs.songs = songs;
                    if count > 0 {
                        state.songs.selected_index = Some(0);
                    } else {
                        state.songs.selected_index = None;
                    }
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
                    let count = songs.len();
                    state.songs.songs = songs;
                    if count > 0 {
                        state.songs.selected_index = Some(0);
                    } else {
                        state.songs.selected_index = None;
                    }
                }
                Err(e) => {
                    error!("Failed to load random songs: {}", e);
                    let mut state = self.state.write().await;
                    state.notify_error(format!("Failed to load random songs: {}", e));
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
