//! Popup for choosing which libraries (music folders) to browse

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::app::state::{AppState, LibraryPicker, RenderMutations};

const HINT: &str = " Space toggle · a all · Enter apply · Esc cancel ";

/// Render the library picker centered over `area`
pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    picker: &LibraryPicker,
    mutations: &mut RenderMutations,
) {
    let colors = *state.settings_state.theme_colors();

    let longest_name = state
        .music_folders
        .iter()
        .map(|f| f.name.chars().count())
        .max()
        .unwrap_or(0);
    // "[x] " prefix plus borders and padding
    let width = (longest_name + 8).max(HINT.chars().count() + 2) as u16;
    let height = state.music_folders.len() as u16 + 2;

    let [popup] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [popup] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(popup);
    mutations.layout.library_picker = Some(popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Libraries ")
        .title_bottom(Line::from(HINT).style(Style::default().fg(colors.muted)))
        .border_style(Style::default().fg(colors.border_focused));

    let items: Vec<ListItem> = state
        .music_folders
        .iter()
        .zip(&picker.checked)
        .map(|(folder, &checked)| {
            let (mark, mark_style) = if checked {
                ("[x] ", Style::default().fg(colors.accent))
            } else {
                ("[ ] ", Style::default().fg(colors.muted))
            };
            ListItem::new(Line::from(vec![
                Span::styled(mark, mark_style),
                Span::styled(folder.name.clone(), Style::default().fg(colors.song)),
            ]))
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(colors.highlight_bg)
            .add_modifier(Modifier::BOLD),
    );

    let mut list_state = ListState::default();
    *list_state.offset_mut() = picker.scroll_offset;
    list_state.select(Some(picker.cursor));

    frame.render_widget(Clear, popup);
    frame.render_stateful_widget(list, popup, &mut list_state);
    mutations.library_picker_scroll_offset = Some(list_state.offset());
}
