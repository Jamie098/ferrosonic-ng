use crossterm::event::{self, KeyCode};

use crate::error::Error;

use super::*;

impl App {
    /// Handle keys while the library picker popup is open
    pub(super) async fn handle_library_picker_key(
        &mut self,
        key: event::KeyEvent,
    ) -> Result<(), Error> {
        let mut state = self.state.write().await;
        let Some(picker) = state.library_picker.as_mut() else {
            return Ok(());
        };

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => picker.move_up(),
            KeyCode::Down | KeyCode::Char('j') => picker.move_down(),
            KeyCode::Char(' ') | KeyCode::Char('x') => picker.toggle(),
            KeyCode::Char('a') => picker.toggle_all(),
            KeyCode::Esc | KeyCode::Char('m') | KeyCode::Char('q') => {
                state.library_picker = None;
            }
            KeyCode::Enter => {
                drop(state);
                return self.apply_library_picker().await;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle a left click while the library picker popup is open: a click on
    /// a row toggles it, a click outside the popup closes it unchanged.
    pub(super) async fn handle_library_picker_click(
        &mut self,
        x: u16,
        y: u16,
    ) -> Result<(), Error> {
        let mut state = self.state.write().await;
        let Some(area) = state.layout.library_picker else {
            return Ok(());
        };

        let inside =
            x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height;
        if !inside {
            state.library_picker = None;
            return Ok(());
        }

        let Some(picker) = state.library_picker.as_mut() else {
            return Ok(());
        };
        // Rows sit between the top and bottom borders
        let on_row = y > area.y && y + 1 < area.y + area.height;
        let item_index = picker.scroll_offset + y.saturating_sub(area.y + 1) as usize;
        if on_row && item_index < picker.checked.len() {
            picker.cursor = item_index;
            picker.toggle();
        }
        Ok(())
    }

    /// Close the library picker and browse the checked libraries
    async fn apply_library_picker(&mut self) -> Result<(), Error> {
        let selection = {
            let mut state = self.state.write().await;
            let Some(picker) = state.library_picker.clone() else {
                return Ok(());
            };
            match state.picker_selection(&picker) {
                Some(ids) => {
                    state.library_picker = None;
                    ids
                }
                None => {
                    state.notify_error("Select at least one library");
                    return Ok(());
                }
            }
        };

        self.set_music_folder_selection(selection).await;
        Ok(())
    }
}
