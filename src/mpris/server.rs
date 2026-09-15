//! MPRIS2 D-Bus server implementation

use mpris_server::{
    zbus::{fdo, Result},
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Property, RootInterface,
    Server, Time, TrackId, Volume,
};
use tokio::sync::mpsc;
use tracing::info;
use url::Url;

use crate::app::actions::AudioAction;
use crate::app::state::{NowPlaying, PlaybackState, SharedState};
use crate::config::Config;
use crate::subsonic::auth::generate_auth_params;
use crate::subsonic::models::Child;

/// API version for Subsonic
const API_VERSION: &str = "1.16.1";
/// Client name for Subsonic
const CLIENT_NAME: &str = "ferrosonic";

/// Build a cover art URL from config and cover art ID
fn build_cover_art_url(config: &Config, cover_art_id: &str) -> Option<String> {
    if config.base_url.is_empty() || cover_art_id.is_empty() {
        return None;
    }

    let (salt, token) = generate_auth_params(&config.password);
    let mut url = Url::parse(&format!("{}/rest/getCoverArt", config.base_url)).ok()?;

    url.query_pairs_mut()
        .append_pair("id", cover_art_id)
        .append_pair("u", &config.username)
        .append_pair("t", &token)
        .append_pair("s", &salt)
        .append_pair("v", API_VERSION)
        .append_pair("c", CLIENT_NAME);

    Some(url.to_string())
}

fn mpris_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn build_metadata(
    now_playing: &NowPlaying,
    current_song: Option<Child>,
    config: &Config,
) -> Metadata {
    let mut metadata = Metadata::new();

    if let Some(song) = current_song {
        metadata.set_trackid(
            Some(TrackId::try_from(format!("/org/mpris/MediaPlayer2/Track/{}", song.id)).ok())
                .flatten(),
        );
        metadata.set_title(Some(song.title));
        metadata.set_artist(song.artist.map(|a| vec![a]));
        metadata.set_album(song.album);

        if let Some(duration) = song.duration {
            metadata.set_length(Some(Time::from_micros(duration as i64 * 1_000_000)));
        }

        if let Some(track) = song.track {
            metadata.set_track_number(Some(track));
        }

        if let Some(disc) = song.disc_number {
            metadata.set_disc_number(Some(disc));
        }

        if let Some(ref cover_art_id) = song.cover_art {
            if let Some(cover_url) = build_cover_art_url(config, cover_art_id) {
                metadata.set_art_url(Some(cover_url));
            }
        }
    } else if let Some(station) = now_playing.radio_station.as_ref() {
        metadata.set_trackid(
            Some(
                TrackId::try_from(format!(
                    "/org/mpris/MediaPlayer2/Radio/{}",
                    mpris_path_segment(&station.id)
                ))
                .ok(),
            )
            .flatten(),
        );
        metadata.set_title(Some(
            now_playing
                .radio_title
                .clone()
                .unwrap_or_else(|| station.name.clone()),
        ));
        metadata.set_artist(Some(vec![now_playing
            .radio_artist
            .clone()
            .unwrap_or_else(|| "Internet Radio".to_string())]));
        metadata.set_album(
            station
                .home_page_url
                .clone()
                .or_else(|| Some(station.name.clone())),
        );

        if let Some(ref cover_art_id) = station.cover_art {
            if let Some(cover_url) = build_cover_art_url(config, cover_art_id) {
                metadata.set_art_url(Some(cover_url));
            }
        }
    }

    metadata
}

/// MPRIS server instance name
const PLAYER_NAME: &str = "ferrosonic";

/// MPRIS2 player implementation
pub struct MprisPlayer {
    state: SharedState,
    audio_tx: mpsc::Sender<AudioAction>,
}

impl MprisPlayer {
    pub fn new(state: SharedState, audio_tx: mpsc::Sender<AudioAction>) -> Self {
        Self { state, audio_tx }
    }

    async fn get_state(&self) -> (NowPlaying, Option<Child>, Config) {
        let state = self.state.read().await;
        let now_playing = state.now_playing.clone();
        let current_song = state.current_song().cloned();
        let config = state.config.clone();
        (now_playing, current_song, config)
    }
}

impl RootInterface for MprisPlayer {
    async fn raise(&self) -> fdo::Result<()> {
        // We're a terminal app, can't raise
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        let mut state = self.state.write().await;
        state.should_quit = true;
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("Ferrosonic".to_string())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("ferrosonic".to_string())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["http".to_string(), "https".to_string()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "audio/mpeg".to_string(),
            "audio/flac".to_string(),
            "audio/ogg".to_string(),
            "audio/wav".to_string(),
            "audio/x-wav".to_string(),
        ])
    }
}

impl PlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        let _ = self.audio_tx.send(AudioAction::Next).await;
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let _ = self.audio_tx.send(AudioAction::Previous).await;
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let _ = self.audio_tx.send(AudioAction::Pause).await;
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let _ = self.audio_tx.send(AudioAction::TogglePause).await;
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        let _ = self.audio_tx.send(AudioAction::Stop).await;
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        let _ = self.audio_tx.send(AudioAction::Resume).await;
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let offset_secs = offset.as_micros() as f64 / 1_000_000.0;
        let _ = self
            .audio_tx
            .send(AudioAction::SeekRelative(offset_secs))
            .await;
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
        let position_secs = position.as_micros() as f64 / 1_000_000.0;
        let _ = self.audio_tx.send(AudioAction::Seek(position_secs)).await;
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        // Not supported for now
        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let (now_playing, _, _) = self.get_state().await;
        Ok(match now_playing.state {
            PlaybackState::Playing => PlaybackStatus::Playing,
            PlaybackState::Paused => PlaybackStatus::Paused,
            PlaybackState::Stopped => PlaybackStatus::Stopped,
        })
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> Result<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let (now_playing, current_song, config) = self.get_state().await;
        Ok(build_metadata(&now_playing, current_song, &config))
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(1.0)
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        let volume_int = (volume * 100.0) as i32;
        let _ = self.audio_tx.send(AudioAction::SetVolume(volume_int)).await;
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        let (now_playing, _, _) = self.get_state().await;
        Ok(Time::from_micros(
            (now_playing.position * 1_000_000.0) as i64,
        ))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        let state = self.state.read().await;
        Ok(state
            .queue_position
            .map(|p| p + 1 < state.queue.len())
            .unwrap_or(false))
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        let state = self.state.read().await;
        Ok(state.queue_position.map(|p| p > 0).unwrap_or(false))
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        let state = self.state.read().await;
        Ok(!state.queue.is_empty() || state.now_playing.radio_station.is_some())
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        let state = self.state.read().await;
        Ok(state.now_playing.radio_station.is_none())
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

/// Start the MPRIS server
pub async fn start_mpris_server(
    state: SharedState,
    audio_tx: mpsc::Sender<AudioAction>,
) -> Result<Server<MprisPlayer>> {
    info!("Starting MPRIS2 server");

    let player = MprisPlayer::new(state, audio_tx);
    let server = Server::new(PLAYER_NAME, player).await?;

    info!(
        "MPRIS2 server started as org.mpris.MediaPlayer2.{}",
        PLAYER_NAME
    );
    Ok(server)
}

/// Snapshot of the last values emitted over MPRIS, used to avoid
/// re-emitting unchanged properties on every mpv event tick.
struct LastEmitted {
    status_code: u8,
    can_go_next: bool,
    can_go_previous: bool,
    can_play: bool,
    can_seek: bool,
    metadata_key: Option<String>,
}

static LAST_EMITTED: std::sync::Mutex<Option<LastEmitted>> = std::sync::Mutex::new(None);

/// Update MPRIS properties when state changes.
///
/// This is called from the mpv event loop, which fires several times per
/// second (position ticks etc.). Emitting `PropertiesChanged` on every call
/// makes desktop widgets re-read state and re-fetch the (possibly remote)
/// cover art each time, causing visible flicker. We therefore remember the
/// last emitted values and only emit player flags when they change and
/// `Metadata` when the track (or radio stream title) changes.
pub async fn update_mpris_properties(
    server: &Server<MprisPlayer>,
    state: &SharedState,
) -> Result<()> {
    let state = state.read().await;

    let status_code: u8 = match state.now_playing.state {
        PlaybackState::Playing => 0,
        PlaybackState::Paused => 1,
        PlaybackState::Stopped => 2,
    };
    let can_go_next = state
        .queue_position
        .map(|p| p + 1 < state.queue.len())
        .unwrap_or(false);
    let can_go_previous = state.queue_position.map(|p| p > 0).unwrap_or(false);
    let can_play = !state.queue.is_empty() || state.now_playing.radio_station.is_some();
    let can_seek = state.now_playing.radio_station.is_none();

    let metadata_key: Option<String> = if let Some(song) = state.current_song() {
        Some(format!("song:{}", song.id))
    } else if let Some(station) = state.now_playing.radio_station.as_ref() {
        Some(format!(
            "radio:{}:{}:{}",
            station.id,
            state.now_playing.radio_title.as_deref().unwrap_or(""),
            state.now_playing.radio_artist.as_deref().unwrap_or(""),
        ))
    } else {
        None
    };

    let (flags_changed, metadata_changed) = {
        let mut last = LAST_EMITTED.lock().unwrap();
        let (flags, meta) = match last.as_ref() {
            Some(prev) => (
                prev.status_code != status_code
                    || prev.can_go_next != can_go_next
                    || prev.can_go_previous != can_go_previous
                    || prev.can_play != can_play
                    || prev.can_seek != can_seek,
                prev.metadata_key != metadata_key,
            ),
            None => (true, true),
        };
        *last = Some(LastEmitted {
            status_code,
            can_go_next,
            can_go_previous,
            can_play,
            can_seek,
            metadata_key: metadata_key.clone(),
        });
        (flags, meta)
    };

    if flags_changed {
        server
            .properties_changed([
                Property::PlaybackStatus(match state.now_playing.state {
                    PlaybackState::Playing => PlaybackStatus::Playing,
                    PlaybackState::Paused => PlaybackStatus::Paused,
                    PlaybackState::Stopped => PlaybackStatus::Stopped,
                }),
                Property::CanGoNext(can_go_next),
                Property::CanGoPrevious(can_go_previous),
                Property::CanPlay(can_play),
                Property::CanSeek(can_seek),
            ])
            .await?;
    }

    if metadata_changed
        && (state.current_song().is_some() || state.now_playing.radio_station.is_some())
    {
        let metadata = build_metadata(
            &state.now_playing,
            state.current_song().cloned(),
            &state.config,
        );
        server
            .properties_changed([Property::Metadata(metadata)])
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::new_shared_state;

    #[tokio::test]
    async fn identity_reports_ferrosonic_name() {
        let (audio_tx, _audio_rx) = mpsc::channel(1);
        let player = MprisPlayer::new(new_shared_state(Config::new()), audio_tx);

        assert_eq!(
            player
                .identity()
                .await
                .expect("identity should be available"),
            "Ferrosonic"
        );
    }
}
