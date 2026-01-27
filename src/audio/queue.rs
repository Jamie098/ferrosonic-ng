//! Play queue management

use rand::seq::SliceRandom;
use tracing::debug;

use crate::subsonic::models::Child;
use crate::subsonic::SubsonicClient;

/// Play queue
#[derive(Debug, Clone, Default)]
pub struct PlayQueue {
    /// Songs in the queue
    songs: Vec<Child>,
    /// Current position in the queue (None = stopped)
    position: Option<usize>,
}

impl PlayQueue {
    /// Create a new empty queue
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the songs in the queue
    pub fn songs(&self) -> &[Child] {
        &self.songs
    }

    /// Get the current position
    pub fn position(&self) -> Option<usize> {
        self.position
    }

    /// Get the current song
    pub fn current(&self) -> Option<&Child> {
        self.position.and_then(|pos| self.songs.get(pos))
    }

    /// Get number of songs in queue
    pub fn len(&self) -> usize {
        self.songs.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    /// Add songs to the end of the queue
    pub fn append(&mut self, songs: impl IntoIterator<Item = Child>) {
        self.songs.extend(songs);
        debug!("Queue now has {} songs", self.songs.len());
    }

    /// Insert songs after the current position
    pub fn insert_next(&mut self, songs: impl IntoIterator<Item = Child>) {
        let insert_pos = self.position.map(|p| p + 1).unwrap_or(0);
        let new_songs: Vec<_> = songs.into_iter().collect();
        let count = new_songs.len();

        for (i, song) in new_songs.into_iter().enumerate() {
            self.songs.insert(insert_pos + i, song);
        }

        debug!("Inserted {} songs at position {}", count, insert_pos);
    }

    /// Clear the queue
    pub fn clear(&mut self) {
        self.songs.clear();
        self.position = None;
        debug!("Queue cleared");
    }

    /// Remove song at index
    pub fn remove(&mut self, index: usize) -> Option<Child> {
        if index >= self.songs.len() {
            return None;
        }

        let song = self.songs.remove(index);

        // Adjust position if needed
        if let Some(pos) = self.position {
            if index < pos {
                self.position = Some(pos - 1);
            } else if index == pos {
                // Removed current song
                if self.songs.is_empty() {
                    self.position = None;
                } else if pos >= self.songs.len() {
                    self.position = Some(self.songs.len() - 1);
                }
            }
        }

        debug!("Removed song at index {}", index);
        Some(song)
    }

    /// Move song from one position to another
    pub fn move_song(&mut self, from: usize, to: usize) {
        if from >= self.songs.len() || to >= self.songs.len() {
            return;
        }

        let song = self.songs.remove(from);
        self.songs.insert(to, song);

        // Adjust position if needed
        if let Some(pos) = self.position {
            if from == pos {
                self.position = Some(to);
            } else if from < pos && to >= pos {
                self.position = Some(pos - 1);
            } else if from > pos && to <= pos {
                self.position = Some(pos + 1);
            }
        }

        debug!("Moved song from {} to {}", from, to);
    }

    /// Shuffle the queue, keeping current song in place
    pub fn shuffle(&mut self) {
        if self.songs.len() <= 1 {
            return;
        }

        let mut rng = rand::thread_rng();

        if let Some(pos) = self.position {
            // Keep current song, shuffle the rest
            let current = self.songs.remove(pos);

            // Shuffle remaining songs
            self.songs.shuffle(&mut rng);

            // Put current song at the front
            self.songs.insert(0, current);
            self.position = Some(0);
        } else {
            // No current song, shuffle everything
            self.songs.shuffle(&mut rng);
        }

        debug!("Queue shuffled");
    }

    /// Set current position
    pub fn set_position(&mut self, position: Option<usize>) {
        if let Some(pos) = position {
            if pos < self.songs.len() {
                self.position = Some(pos);
                debug!("Position set to {}", pos);
            }
        } else {
            self.position = None;
            debug!("Position cleared");
        }
    }

    /// Advance to next song
    /// Returns true if there was a next song
    pub fn next(&mut self) -> bool {
        match self.position {
            Some(pos) if pos + 1 < self.songs.len() => {
                self.position = Some(pos + 1);
                debug!("Advanced to position {}", pos + 1);
                true
            }
            _ => {
                self.position = None;
                debug!("Reached end of queue");
                false
            }
        }
    }

    /// Go to previous song
    /// Returns true if there was a previous song
    pub fn previous(&mut self) -> bool {
        match self.position {
            Some(pos) if pos > 0 => {
                self.position = Some(pos - 1);
                debug!("Went back to position {}", pos - 1);
                true
            }
            _ => {
                if !self.songs.is_empty() {
                    self.position = Some(0);
                }
                debug!("At start of queue");
                false
            }
        }
    }

    /// Get stream URL for current song
    pub fn current_stream_url(&self, client: &SubsonicClient) -> Option<String> {
        self.current()
            .and_then(|song| client.get_stream_url(&song.id).ok())
    }

    /// Get stream URL for song at index
    pub fn stream_url_at(&self, index: usize, client: &SubsonicClient) -> Option<String> {
        self.songs
            .get(index)
            .and_then(|song| client.get_stream_url(&song.id).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_song(id: &str, title: &str) -> Child {
        Child {
            id: id.to_string(),
            title: title.to_string(),
            parent: None,
            is_dir: false,
            album: None,
            artist: None,
            track: None,
            year: None,
            genre: None,
            cover_art: None,
            size: None,
            content_type: None,
            suffix: None,
            duration: None,
            bit_rate: None,
            path: None,
            disc_number: None,
        }
    }

    #[test]
    fn test_append_and_len() {
        let mut queue = PlayQueue::new();
        assert!(queue.is_empty());

        queue.append(vec![make_song("1", "Song 1"), make_song("2", "Song 2")]);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_position_and_navigation() {
        let mut queue = PlayQueue::new();
        queue.append(vec![
            make_song("1", "Song 1"),
            make_song("2", "Song 2"),
            make_song("3", "Song 3"),
        ]);

        assert!(queue.current().is_none());

        queue.set_position(Some(0));
        assert_eq!(queue.current().unwrap().id, "1");

        assert!(queue.next());
        assert_eq!(queue.current().unwrap().id, "2");

        assert!(queue.next());
        assert_eq!(queue.current().unwrap().id, "3");

        assert!(!queue.next());
        assert!(queue.current().is_none());
    }

    #[test]
    fn test_remove() {
        let mut queue = PlayQueue::new();
        queue.append(vec![
            make_song("1", "Song 1"),
            make_song("2", "Song 2"),
            make_song("3", "Song 3"),
        ]);
        queue.set_position(Some(1));

        // Remove song before current
        queue.remove(0);
        assert_eq!(queue.position(), Some(0));
        assert_eq!(queue.current().unwrap().id, "2");

        // Remove current song
        queue.remove(0);
        assert_eq!(queue.current().unwrap().id, "3");
    }

    #[test]
    fn test_insert_next() {
        let mut queue = PlayQueue::new();
        queue.append(vec![make_song("1", "Song 1"), make_song("3", "Song 3")]);
        queue.set_position(Some(0));

        queue.insert_next(vec![make_song("2", "Song 2")]);

        assert_eq!(queue.songs[0].id, "1");
        assert_eq!(queue.songs[1].id, "2");
        assert_eq!(queue.songs[2].id, "3");
    }

    #[test]
    fn test_move_song() {
        let mut queue = PlayQueue::new();
        queue.append(vec![
            make_song("1", "Song 1"),
            make_song("2", "Song 2"),
            make_song("3", "Song 3"),
        ]);
        queue.set_position(Some(0));

        queue.move_song(0, 2);
        assert_eq!(queue.songs[0].id, "2");
        assert_eq!(queue.songs[1].id, "3");
        assert_eq!(queue.songs[2].id, "1");
        assert_eq!(queue.position(), Some(2));
    }
}
