use super::*;
use crate::error::Error;
use ::ratatui::prelude::{Constraint, Layout};
use strum::IntoEnumIterator;

impl App {
    pub(super) async fn handle_browse_click(
        &mut self,
        x: u16,
        y: u16,
        layout: &LayoutAreas,
    ) -> Result<(), Error> {
        let content = layout.content;

        // Match the Browse page render layout: Tab bar | Options | Search | Content
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(content);

        let tab_area = chunks[0];
        let options_area = chunks[1];
        let search_area = chunks[2];
        let list_area = chunks[3];

        let browse_tab = self.state.read().await.browse.browse_tab.clone();

        if y >= tab_area.y && y < tab_area.y + tab_area.height {
            let Some(new_tab) = crate::ui::pages::browse::tab_at_column(tab_area.x, x) else {
                self.last_click = Some((x, y, std::time::Instant::now()));
                return Ok(());
            };

            let mut state = self.state.write().await;
            if state.browse.browse_tab != new_tab {
                state.browse.browse_tab = new_tab.clone();
                state.browse.filter.clear();
                state.browse.focus = 0;
                let selected_option = state.browse.selected_option.clone();
                drop(state);
                self.songs_filter_debounce = None;

                let opt = selected_option.unwrap_or(SongOption::All);
                match new_tab {
                    BrowseTab::Songs => match opt {
                        SongOption::All => {
                            let mut state = self.state.write().await;
                            state.browse.all_songs_offset = 0;
                            state.browse.all_songs_has_more = true;
                            drop(state);
                            self.get_all_songs(false).await;
                        }
                        SongOption::Starred => self.get_starred_songs().await,
                        SongOption::Random => self.get_random_songs().await,
                    },
                    BrowseTab::Albums => match opt {
                        SongOption::All => {
                            let mut state = self.state.write().await;
                            state.browse.albums_offset = 0;
                            state.browse.albums_has_more = true;
                            drop(state);
                            self.get_browse_albums(false).await;
                        }
                        SongOption::Starred => self.get_starred_albums().await,
                        SongOption::Random => self.get_random_albums().await,
                    },
                }
            }
        } else if y >= options_area.y && y < options_area.y + options_area.height {
            let row = y.saturating_sub(options_area.y + 1) as usize;
            let mut state = self.state.write().await;
            state.browse.focus = 0;

            if let Some(opt) = SongOption::iter().nth(row) {
                if state.browse.selected_option.as_ref() != Some(&opt) {
                    state.browse.selected_option = Some(opt.clone());
                    state.browse.scroll_offset = 0;
                    state.browse.album_scroll_offset = 0;
                    let tab = state.browse.browse_tab.clone();
                    drop(state);
                    match tab {
                        BrowseTab::Songs => match opt {
                            SongOption::All => {
                                let mut state = self.state.write().await;
                                state.browse.all_songs_offset = 0;
                                state.browse.all_songs_has_more = true;
                                drop(state);
                                self.get_all_songs(false).await;
                            }
                            SongOption::Starred => self.get_starred_songs().await,
                            SongOption::Random => self.get_random_songs().await,
                        },
                        BrowseTab::Albums => match opt {
                            SongOption::All => {
                                let mut state = self.state.write().await;
                                state.browse.albums_offset = 0;
                                state.browse.albums_has_more = true;
                                drop(state);
                                self.get_browse_albums(false).await;
                            }
                            SongOption::Starred => self.get_starred_albums().await,
                            SongOption::Random => self.get_random_albums().await,
                        },
                    }
                }
            }
        } else if y >= search_area.y && y < search_area.y + search_area.height {
            self.state.write().await.browse.filter_active = true;
        } else if y >= list_area.y && y < list_area.y + list_area.height {
            let row_in_viewport = y.saturating_sub(list_area.y + 1) as usize;

            match browse_tab {
                BrowseTab::Songs => {
                    let mut state = self.state.write().await;
                    let item_index = state.browse.scroll_offset + row_in_viewport;

                    if item_index < state.browse.songs.len() {
                        let was_selected = state.browse.selected_index == Some(item_index);
                        state.browse.focus = 1;
                        state.browse.selected_index = Some(item_index);

                        let is_second_click = was_selected
                            && self.last_click.is_some_and(|(lx, ly, t)| {
                                lx == x && ly == y && t.elapsed().as_millis() < 500
                            });

                        if is_second_click {
                            let songs = state.browse.songs.clone();
                            state.queue.clear();
                            state.queue.extend(songs);
                            drop(state);
                            self.last_click = Some((x, y, std::time::Instant::now()));
                            return self.play_queue_position(item_index).await;
                        }
                    }
                }
                BrowseTab::Albums => {
                    let mut state = self.state.write().await;
                    let item_index = state.browse.album_scroll_offset + row_in_viewport;

                    if item_index < state.browse.albums.len() {
                        let was_selected = state.browse.selected_album == Some(item_index);
                        state.browse.focus = 1;
                        state.browse.selected_album = Some(item_index);

                        let is_second_click = was_selected
                            && self.last_click.is_some_and(|(lx, ly, t)| {
                                lx == x && ly == y && t.elapsed().as_millis() < 500
                            });

                        if is_second_click {
                            let album_id = state.browse.albums[item_index].id.clone();
                            let album_name = state.browse.albums[item_index].name.clone();
                            drop(state);
                            self.last_click = Some((x, y, std::time::Instant::now()));

                            if let Some(ref client) = self.subsonic {
                                match client.get_album(&album_id).await {
                                    Ok((_album, songs)) => {
                                        let count = songs.len();
                                        let mut state = self.state.write().await;
                                        state.queue.clear();
                                        state.queue.extend(songs);
                                        state.notify(format!(
                                            "Playing: {} ({} songs)",
                                            album_name, count
                                        ));
                                        drop(state);
                                        return self.play_queue_position(0).await;
                                    }
                                    Err(e) => {
                                        self.state
                                            .write()
                                            .await
                                            .notify_error(format!("Failed to load album: {}", e));
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }
                }
            }
        }

        self.last_click = Some((x, y, std::time::Instant::now()));
        Ok(())
    }
}
