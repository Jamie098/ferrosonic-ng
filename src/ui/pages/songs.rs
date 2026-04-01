use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::state::AppState;
use crate::ui::theme::ThemeColors;

pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let colors = *state.settings_state.theme_colors();

    let chunks =
        Layout::vertical([Constraint::Percentage(15), Constraint::Percentage(85)]).split(area);

    render_options(frame, chunks[0], state, &colors);
    render_songs(frame, chunks[1], state, &colors);
}

fn render_options(frame: &mut Frame, area: Rect, state: &mut AppState, colors: &ThemeColors) {
    let songs = &state.songs;

    let focused = songs.focus == 0;
    let border_style = if focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Song Options")
        .border_style(border_style);

    let items = songs.options.iter().enumerate().map(|(i, option)| {
        let is_selected = Some(i) == songs.selected_option && focused;
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
    if focused {
        list = list.highlight_style(
            Style::default()
                .bg(colors.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );
    };

    let mut list_state = ListState::default();
    list_state.select(state.songs.selected_option);

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_songs(frame: &mut Frame, area: Rect, state: &mut AppState, colors: &ThemeColors) {
    let songs = &state.songs;

    let focused = songs.focus == 1;
    let border_style = if focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border_unfocused)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Songs")
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

            let indicator = if is_playing { "▶ " } else { "  " };

            let (title_color, time_color) = if is_selected {
                (colors.highlight_fg, colors.highlight_fg)
            } else if is_playing {
                (colors.playing, colors.muted)
            } else {
                (colors.song, colors.muted)
            };

            let artist = match &song.artist {
                Some(value) => value,
                None => "n/a",
            };

            let line = Line::from(vec![
                Span::styled(indicator.to_string(), Style::default().fg(colors.playing)),
                Span::styled(song.title.clone(), Style::default().fg(title_color)),
                Span::styled(format!(" - {}", artist), Style::default().fg(title_color)),
                Span::styled(
                    format!(" [{}]", song.format_duration()),
                    Style::default().fg(time_color),
                ),
            ]);

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
