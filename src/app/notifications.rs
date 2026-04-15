use notify_rust::{Hint, Notification, Timeout};
use std::sync::Mutex;
use tracing::error;

/// Holds the ID of the last notification so we can replace it
static LAST_NOTIFICATION_ID: Mutex<Option<u32>> = Mutex::new(None);

pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
}

pub fn notify_track_change(track: &TrackInfo) {
    let mut builder = Notification::new();
    builder
        .appname("Ferrosonic")
        .summary(&track.title)
        .body(&format!("{} — {}", track.artist, track.album))
        .hint(Hint::Category("music".to_owned()))
        .hint(Hint::Transient(true)) // don't persist in notification center
        .timeout(Timeout::Milliseconds(5000));

    // Replace the previous notification instead of stacking
    let previous_id = match LAST_NOTIFICATION_ID.lock() {
        Ok(last_id) => *last_id,
        Err(poisoned) => {
            error!("LAST_NOTIFICATION_ID lock poisoned while reading previous notification ID");
            *poisoned.into_inner()
        }
    };

    if let Some(id) = previous_id {
        builder.id(id);
    }

    match builder.show() {
        Ok(handle) => match LAST_NOTIFICATION_ID.lock() {
            Ok(mut last_id) => *last_id = Some(handle.id()),
            Err(poisoned) => {
                error!("LAST_NOTIFICATION_ID lock poisoned while storing notification ID");
                *poisoned.into_inner() = Some(handle.id());
            }
        },
        Err(e) => error!("Failed to show notification: {}", e),
    }
}
