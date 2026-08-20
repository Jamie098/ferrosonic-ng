use std::time::Duration;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::state::{AppState, RenderMutations};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, mutations: &mut RenderMutations) {
    let colors = state.settings_state.theme_colors();
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Lyrics ")
        .border_style(Style::default().fg(colors.border_focused));

    if state.now_playing.song.is_none() {
        let p = Paragraph::new("No song playing.")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(p, area);
        return;
    }

    let parsed = match &state.now_playing.parsed_lyrics {
        Some(p) => p,
        None => {
            let msg = state.now_playing.lyrics.as_deref().unwrap_or("No lyrics available.");
            let p = Paragraph::new(msg)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(p, area);
            return;
        }
    };

    if parsed.lines.is_empty() {
        let p = Paragraph::new("Lyrics are empty.")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(p, area);
        return;
    }

    let current_pos = Duration::from_secs_f64(state.now_playing.position);
    let current_idx = parsed.current_index(current_pos);
    let spacing = state.lyrics_state.line_spacing as usize;

    // Auto-scroll logic: if synced and not manually scrolling (or manual timeout)
    if parsed.is_synced && !state.lyrics_state.is_manual_scroll {
        if let Some(idx) = current_idx {
            // Center the current line
            // Account for spacing (each line takes 1 + spacing actual rows)
            let line_height = 1 + spacing;
            let center_offset = (area.height.saturating_sub(2) / 2) as usize;
            mutations.lyrics_scroll_offset = Some((idx * line_height).saturating_sub(center_offset));
        }
    } else if state.lyrics_state.is_manual_scroll {
        // Auto-resume after 5 seconds of inactivity if synced
        if let Some(last) = state.lyrics_state.last_scroll_time {
            if last.elapsed() > Duration::from_secs(5) && parsed.is_synced {
                mutations.lyrics_reset_manual_scroll = Some(false);
            }
        }
    }

    let mut lines = Vec::new();
    for (i, line) in parsed.lines.iter().enumerate() {
        let is_current = current_idx == Some(i);
        
        let style = if is_current {
            Style::default()
                .fg(colors.accent) 
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.muted)
        };

        let span = Span::styled(&line.text, style);
        lines.push(Line::from(vec![span]).alignment(Alignment::Center));

        // Add empty lines for spacing
        for _ in 0..spacing {
            lines.push(Line::default());
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((state.lyrics_state.scroll_offset.try_into().unwrap(), 0));

    frame.render_widget(paragraph, area);
}
