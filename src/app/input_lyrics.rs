use std::time::Instant;
use crossterm::event::{KeyCode, KeyModifiers};

use crate::error::Error;

use super::*;

impl App {
    pub(super) async fn handle_lyrics_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut state = self.state.write().await;

        match (key.code, key.modifiers) {
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                if state.lyrics_state.scroll_offset > 0 {
                    state.lyrics_state.scroll_offset -= 1;
                }
                state.lyrics_state.is_manual_scroll = true;
                state.lyrics_state.last_scroll_time = Some(Instant::now());
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.lyrics_state.scroll_offset += 1;
                state.lyrics_state.is_manual_scroll = true;
                state.lyrics_state.last_scroll_time = Some(Instant::now());
            }
            (KeyCode::PageUp, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                state.lyrics_state.scroll_offset = state.lyrics_state.scroll_offset.saturating_sub(10);
                state.lyrics_state.is_manual_scroll = true;
                state.lyrics_state.last_scroll_time = Some(Instant::now());
            }
            (KeyCode::PageDown, _) | (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                state.lyrics_state.scroll_offset = state.lyrics_state.scroll_offset.saturating_add(10);
                state.lyrics_state.is_manual_scroll = true;
                state.lyrics_state.last_scroll_time = Some(Instant::now());
            }
            (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                state.lyrics_state.scroll_offset = 0;
                state.lyrics_state.is_manual_scroll = false;
                state.lyrics_state.last_scroll_time = None;
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) => {
                state.lyrics_state.is_manual_scroll = false;
                state.lyrics_state.last_scroll_time = None;
            }
            (KeyCode::Char(']'), _) => {
                if state.lyrics_state.line_spacing < 5 {
                    state.lyrics_state.line_spacing += 1;
                }
            }
            (KeyCode::Char('['), _) => {
                if state.lyrics_state.line_spacing > 0 {
                    state.lyrics_state.line_spacing -= 1;
                }
            }
            _ => {}
        }

        Ok(())
    }
}
