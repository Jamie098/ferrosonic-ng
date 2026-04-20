use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::models::SongOption;
use crate::app::state::AppState;
use crate::ui::styled_lines::get_song_with_artist_line;
use crate::ui::theme::ThemeColors;
use strum::IntoEnumIterator;

pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let colors = *state.settings_state.theme_colors();

    let chunks = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .split(area);

    render_options(frame, chunks[0], state, &colors);
    render_search(frame, chunks[1], state, &colors);
    render_songs(frame, chunks[2], state, &colors);
}

fn render_options(frame: &mut Frame, area: Rect, state: &mut AppState, colors: &ThemeColors) {
    let focus = state.songs.focus;
    let selected_option = state
        .songs
        .selected_option
        .clone()
        .unwrap_or(SongOption::All);

    let focused = focus == 0;
    let border_style = if focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let title = " Song Options ".to_string();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let items = SongOption::iter().map(|option| {
        let is_selected = option == selected_option;

        let title_color = if is_selected {
            colors.highlight_fg
        } else {
            colors.song
        };

        ListItem::new(Span::styled(
            option.to_string(),
            Style::default().fg(title_color),
        ))
    });

    let mut list = List::new(items).block(block);
    let mut highlight_style = Style::default().add_modifier(Modifier::BOLD);

    if focused {
        highlight_style = highlight_style.bg(colors.highlight_bg);
    };

    list = list.highlight_style(highlight_style);

    let mut list_state = ListState::default();
    list_state.select(Some(selected_option as usize));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_search(frame: &mut Frame, area: Rect, state: &mut AppState, colors: &ThemeColors) {
    let border_style = if state.songs.filter_active {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let block = Block::bordered().title("Search").border_style(border_style);

    let paragraph = Paragraph::new(state.songs.filter.as_str()).block(block);
    frame.render_widget(paragraph, area);
}

fn render_songs(frame: &mut Frame, area: Rect, state: &mut AppState, colors: &ThemeColors) {
    let songs = &state.songs;

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
    *list_state.offset_mut() = state.songs.scroll_offset;
    if focused {
        list_state.select(state.songs.selected_index);
    }

    frame.render_stateful_widget(list, area, &mut list_state);
    state.songs.scroll_offset = list_state.offset();
}
