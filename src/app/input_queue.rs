use crossterm::event::{self, KeyCode};

use crate::error::Error;

use super::*;

impl App {
    /// Handle queue page keys
    pub(super) async fn handle_queue_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
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
            KeyCode::Char('s') => {
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
            // f: star / un-star
            KeyCode::Char('f') => {
                let selected_song_idx = state.queue_state.selected;

                let Some(selected_song_idx) = selected_song_idx else {
                    return Ok(());
                };

                let song = &mut state.queue[selected_song_idx];
                let id = song.id.clone();
                let was_starred = song.starred.is_some();
                let new_starred = if was_starred {
                    None
                } else {
                    Some("starred".to_string())
                };
                song.starred = new_starred.clone();
                state.browse.starred_songs_dirty = true;

                drop(state);

                if was_starred {
                    self.unstar_song(id).await;
                } else {
                    self.star_song(id).await;
                }
            }
            _ => {}
        }

        Ok(())
    }
}
