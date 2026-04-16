//! Settings page with app preferences and theming

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::state::AppState;
use crate::ui::theme::ThemeColors;

/// Render the settings page
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let colors = *state.settings_state.theme_colors();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Settings ")
        .border_style(Style::default().fg(colors.border_focused));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 15 {
        return;
    }

    let settings = &state.settings_state;

    // Layout fields vertically with spacing
    let chunks = Layout::vertical([
        Constraint::Length(1), // Spacing
        Constraint::Length(2), // Theme selector
        Constraint::Length(1), // Spacing
        Constraint::Length(2), // Cava toggle
        Constraint::Length(1), // Spacing
        Constraint::Length(2), // Cava size
        Constraint::Length(1), // Spacing
        Constraint::Length(2), // Notifications
        Constraint::Length(1), // Spacing
        Constraint::Length(2), // Scrobble toggle
        Constraint::Min(1),    // Remaining space
    ])
    .split(inner);

    // Theme selector (field 0)
    render_option(
        frame,
        chunks[1],
        "Theme",
        settings.theme_name(),
        settings.selected_field == 0,
        &colors,
    );

    // Cava toggle (field 1)
    let cava_value = if !state.cava_available {
        "Off (cava not found)"
    } else if settings.cava_enabled {
        "On"
    } else {
        "Off"
    };

    render_option(
        frame,
        chunks[3],
        "Cava Visualizer",
        cava_value,
        settings.selected_field == 1,
        &colors,
    );

    // Cava size (field 2)
    let cava_size_value = if !state.cava_available {
        "N/A (cava not found)".to_string()
    } else {
        format!("{}%", settings.cava_size)
    };

    render_option(
        frame,
        chunks[5],
        "Cava Size",
        &cava_size_value,
        settings.selected_field == 2,
        &colors,
    );

    // Notifications toggle (field 3)
    let notifications_value = if settings.notifications_enabled {
        "On"
    } else {
        "Off"
    };

    render_option(
        frame,
        chunks[7],
        "Notifications",
        notifications_value,
        settings.selected_field == 3,
        &colors,
    );

    // Scrobble toggle (field 4)
    let scrobble_value = if settings.scrobble_enabled {
        "On"
    } else {
        "Off"
    };

    render_option(
        frame,
        chunks[9],
        "Scrobble",
        scrobble_value,
        settings.selected_field == 4,
        &colors,
    );

    // Help text at bottom
    let help_text = match settings.selected_field {
        0 => "← → or Enter to change theme (auto-saves)",
        1 if state.cava_available => "← → or Enter to toggle cava visualizer (auto-saves)",
        1 => "cava is not installed on this system",
        2 if state.cava_available => "← → to adjust cava size (10%-80%, auto-saves)",
        2 => "cava is not installed on this system",
        3 => "← → or Enter to toggle desktop notifications",
        4 => "← → or Enter to toggle scrobbling played tracks to the server",
        _ => "",
    };
    let help = Paragraph::new(help_text).style(Style::default().fg(colors.muted));

    let help_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(2),
        inner.width,
        1,
    );
    frame.render_widget(help, help_area);
}

/// Render an option selector
fn render_option(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    selected: bool,
    colors: &ThemeColors,
) {
    let label_style = if selected {
        Style::default()
            .fg(colors.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.highlight_fg)
    };

    let value_style = if selected {
        Style::default().fg(colors.accent)
    } else {
        Style::default().fg(colors.muted)
    };

    // Label
    let label_text = Paragraph::new(label).style(label_style);
    frame.render_widget(label_text, Rect::new(area.x, area.y, area.width, 1));

    // Value with arrows
    let value_text = if selected {
        format!("  ◀ {} ▶", value)
    } else {
        format!("    {}", value)
    };

    let value_para = Paragraph::new(value_text).style(value_style);
    frame.render_widget(value_para, Rect::new(area.x, area.y + 1, area.width, 1));
}
