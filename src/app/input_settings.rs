use crossterm::event::{self, KeyCode};

use crate::error::Error;

use super::*;

impl App {
    /// Handle settings page keys
    pub(super) async fn handle_settings_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut config_changed = false;

        {
            let mut state = self.state.write().await;
            let field = state.settings_state.selected_field;

            match key.code {
                // Navigate between fields
                KeyCode::Up | KeyCode::Char('k') => {
                    if field > 0 {
                        state.settings_state.selected_field = field - 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if field < 2 {
                        state.settings_state.selected_field = field + 1;
                    }
                }
                // Left
                KeyCode::Left | KeyCode::Char('h') => match field {
                    0 => {
                        state.settings_state.prev_theme();
                        state.config.theme = state.settings_state.theme_name().to_string();
                        let label = state.settings_state.theme_name().to_string();
                        state.notify(format!("Theme: {}", label));
                        config_changed = true;
                    }
                    1 if state.cava_available => {
                        state.settings_state.cava_enabled = !state.settings_state.cava_enabled;
                        state.config.cava = state.settings_state.cava_enabled;
                        let status = if state.settings_state.cava_enabled {
                            "On"
                        } else {
                            "Off"
                        };
                        state.notify(format!("Cava: {}", status));
                        config_changed = true;
                    }
                    2 if state.cava_available => {
                        let cur = state.settings_state.cava_size;
                        if cur > 10 {
                            let new_size = cur - 5;
                            state.settings_state.cava_size = new_size;
                            state.config.cava_size = new_size;
                            state.notify(format!("Cava Size: {}%", new_size));
                            config_changed = true;
                        }
                    }
                    _ => {}
                },
                // Right / Enter / Space
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                    match field {
                        0 => {
                            state.settings_state.next_theme();
                            state.config.theme = state.settings_state.theme_name().to_string();
                            let label = state.settings_state.theme_name().to_string();
                            state.notify(format!("Theme: {}", label));
                            config_changed = true;
                        }
                        1 if state.cava_available => {
                            state.settings_state.cava_enabled = !state.settings_state.cava_enabled;
                            state.config.cava = state.settings_state.cava_enabled;
                            let status = if state.settings_state.cava_enabled {
                                "On"
                            } else {
                                "Off"
                            };
                            state.notify(format!("Cava: {}", status));
                            config_changed = true;
                        }
                        2 if state.cava_available => {
                            let cur = state.settings_state.cava_size;
                            if cur < 80 {
                                let new_size = cur + 5;
                                state.settings_state.cava_size = new_size;
                                state.config.cava_size = new_size;
                                state.notify(format!("Cava Size: {}%", new_size));
                                config_changed = true;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if config_changed {
            // Save config
            let state = self.state.read().await;
            if let Err(e) = state.config.save_default() {
                drop(state);
                let mut state = self.state.write().await;
                state.notify_error(format!("Failed to save: {}", e));
            } else {
                // Start/stop cava based on new setting, or restart on theme change
                let cava_enabled = state.settings_state.cava_enabled;
                let td = state.settings_state.current_theme();
                let g = td.cava_gradient.clone();
                let h = td.cava_horizontal_gradient.clone();
                let cs = state.settings_state.cava_size as u32;
                let cava_running = self.cava_parser.is_some();
                drop(state);
                if cava_enabled {
                    // (Re)start cava — picks up new theme colors or toggle-on
                    self.start_cava(&g, &h, cs);
                } else if cava_running {
                    self.stop_cava();
                    let mut state = self.state.write().await;
                    state.cava_screen.clear();
                }
            }
        }

        Ok(())
    }
}
