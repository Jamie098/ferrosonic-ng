use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use crate::app::models::{BrowseTab, SongOption};
use crate::app::state::AppState;
use crate::ui::styled_lines::{get_album_line, get_song_with_artist_line};
use crate::ui::theme::ThemeColors;
use strum::IntoEnumIterator;

pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let colors = *state.settings_state.theme_colors();

    let chunks = Layout::vertical([
        Constraint::Length(3), // Tab bar
        Constraint::Length(5), // Options (3 items + borders)
        Constraint::Length(3), // Search
        Constraint::Fill(1),   // Content list
    ])
    .split(area);

    render_tabs(frame, chunks[0], state, &colors);
    render_options(frame, chunks[1], state, &colors);
    render_search(frame, chunks[2], state, &colors);

    match state.browse.browse_tab {
        BrowseTab::Songs => render_songs(frame, chunks[3], state, &colors),
        BrowseTab::Albums => render_albums(frame, chunks[3], state, &colors),
    }
}

/// Tab titles in display order — shared with mouse hit-testing so both stay in sync.
pub const TAB_TITLES: &[(BrowseTab, &str)] =
    &[(BrowseTab::Songs, "Songs"), (BrowseTab::Albums, "Albums")];
/// Divider string used between tab titles by the `Tabs` widget.
pub const TAB_DIVIDER: &str = " │ ";

fn render_tabs(frame: &mut Frame, area: Rect, state: &AppState, colors: &ThemeColors) {
    let titles: Vec<Line> = TAB_TITLES.iter().map(|(_, t)| Line::from(*t)).collect();
    let selected = TAB_TITLES
        .iter()
        .position(|(tab, _)| *tab == state.browse.browse_tab)
        .unwrap_or(0);
    let block = Block::default().borders(Borders::ALL);
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(block)
        .style(Style::default().fg(colors.muted))
        .highlight_style(
            Style::default()
                .fg(colors.highlight_fg)
                .bg(colors.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .divider(TAB_DIVIDER);
    frame.render_widget(tabs, area);
}

/// Compute which tab, if any, contains column `x` relative to the tab bar's left edge.
///
/// Mirrors ratatui's `Tabs` layout: a leading space inside the border, then each
/// title surrounded by one space on either side, separated by the divider string.
pub fn tab_at_column(tab_area_x: u16, x: u16) -> Option<BrowseTab> {
    let mut cursor = tab_area_x + 1; // border
    for (i, (tab, title)) in TAB_TITLES.iter().enumerate() {
        if i > 0 {
            cursor += TAB_DIVIDER.chars().count() as u16;
        }
        cursor += 1; // leading space padding
        let title_len = title.chars().count() as u16;
        if x >= cursor && x < cursor + title_len {
            return Some(*tab);
        }
        cursor += title_len;
        cursor += 1; // trailing space padding
    }
    None
}

fn render_options(frame: &mut Frame, area: Rect, state: &AppState, colors: &ThemeColors) {
    let focused = state.browse.focus == 0;
    let selected_option = state
        .browse
        .selected_option
        .clone()
        .unwrap_or(SongOption::All);

    let border_style = if focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let title = match state.browse.browse_tab {
        BrowseTab::Songs => " Song Options ",
        BrowseTab::Albums => " Album Options ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let items = SongOption::iter().map(|option| {
        let is_selected = option == selected_option;
        let color = if is_selected {
            colors.highlight_fg
        } else {
            colors.song
        };
        ListItem::new(Span::styled(option.to_string(), Style::default().fg(color)))
    });

    let mut list = List::new(items).block(block);
    let mut highlight_style = Style::default().add_modifier(Modifier::BOLD);
    if focused {
        highlight_style = highlight_style.bg(colors.highlight_bg);
    }
    list = list.highlight_style(highlight_style);

    let mut list_state = ListState::default();
    list_state.select(Some(selected_option as usize));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_search(frame: &mut Frame, area: Rect, state: &AppState, colors: &ThemeColors) {
    let border_style = if state.browse.filter_active {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let block = Block::bordered().title("Search").border_style(border_style);
    let paragraph = Paragraph::new(state.browse.filter.as_str()).block(block);
    frame.render_widget(paragraph, area);
}

fn render_songs(frame: &mut Frame, area: Rect, state: &mut AppState, colors: &ThemeColors) {
    let songs = &state.browse;
    let focused = songs.focus == 1;

    let border_style = if focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let title = if songs.all_songs_loading {
        " Songs [loading…] ".to_string()
    } else {
        " Songs ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let items: Vec<ListItem> = songs
        .songs
        .iter()
        .enumerate()
        .map(|(i, song)| {
            let is_selected = Some(i) == songs.selected_index && focused;
            let is_playing = state
                .current_song()
                .map(|s| s.id == song.id)
                .unwrap_or(false);
            let line = get_song_with_artist_line(&song, is_selected, is_playing, &colors);
            ListItem::new(line)
        })
        .collect();

    let mut list = List::new(items).block(block);
    if focused {
        list = list.highlight_style(
            Style::default()
                .bg(colors.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );
    }

    let mut list_state = ListState::default();
    *list_state.offset_mut() = state.browse.scroll_offset;
    if focused {
        list_state.select(state.browse.selected_index);
    }

    frame.render_stateful_widget(list, area, &mut list_state);
    state.browse.scroll_offset = list_state.offset();
}

fn render_albums(frame: &mut Frame, area: Rect, state: &mut AppState, colors: &ThemeColors) {
    let browse_state = &state.browse;
    let focused = browse_state.focus == 1;

    let border_style = if focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let title = if browse_state.albums_loading {
        " Albums [loading…] ".to_string()
    } else {
        " Albums ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let items: Vec<ListItem> = browse_state
        .albums
        .iter()
        .enumerate()
        .map(|(i, album)| {
            let is_selected = Some(i) == browse_state.selected_album && focused;
            let line = get_album_line(album, is_selected, colors);
            ListItem::new(line)
        })
        .collect();

    let mut list = List::new(items).block(block);
    if focused {
        list = list.highlight_style(
            Style::default()
                .bg(colors.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );
    }

    let mut list_state = ListState::default();
    *list_state.offset_mut() = state.browse.album_scroll_offset;
    if focused {
        list_state.select(state.browse.selected_album);
    }

    frame.render_stateful_widget(list, area, &mut list_state);
    state.browse.album_scroll_offset = list_state.offset();
}
