//! Footer bar with keybind hints and status

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

use crate::app::state::{Notification, Page, RepeatMode};
use crate::ui::theme::ThemeColors;

/// Footer bar widget
pub struct Footer<'a> {
    page: Page,
    sample_rate: Option<u32>,
    notification: Option<&'a Notification>,
    repeat_mode: RepeatMode,
    volume: u8,
    colors: ThemeColors,
}

impl<'a> Footer<'a> {
    pub fn new(page: Page, colors: ThemeColors) -> Self {
        Self {
            page,
            sample_rate: None,
            notification: None,
            repeat_mode: RepeatMode::Off,
            volume: 100,
            colors,
        }
    }

    pub fn sample_rate(mut self, rate: Option<u32>) -> Self {
        self.sample_rate = rate;
        self
    }

    pub fn notification(mut self, notification: Option<&'a Notification>) -> Self {
        self.notification = notification;
        self
    }

    pub fn repeat_mode(mut self, mode: RepeatMode) -> Self {
        self.repeat_mode = mode;
        self
    }

    pub fn volume(mut self, volume: u8) -> Self {
        self.volume = volume;
        self
    }

    fn keybinds(&self) -> Vec<(&'static str, &'static str)> {
        let mut binds = vec![
            ("q", "Quit"),
            ("p/Space", "Pause"),
            ("h", "Prev"),
            ("l", "Next"),
            ("r", "Repeat"),
            ("+/-", "Volume"),
        ];

        match self.page {
            Page::Browse => {
                binds.extend([
                    ("Enter", "Play"),
                    ("e", "Add"),
                    ("n", "Add next"),
                    ("←/→", "Songs/Albums"),
                    ("/", "Search"),
                    ("Tab", "Focus"),
                    ("f", "Star/Un-star"),
                ]);
            }
            Page::Artists => {
                binds.extend([
                    ("Enter", "Play"),
                    ("←/→", "Focus"),
                    ("/", "Search"),
                    ("e", "Add"),
                    ("n", "Add next"),
                    ("s", "Shuffle"),
                    ("f", "Star/Un-star"),
                ]);
            }
            Page::Queue => {
                binds.extend([
                    ("Enter", "Play"),
                    ("d", "Remove"),
                    ("J/K", "Move"),
                    ("s", "Shuffle"),
                    ("c", "Clear history"),
                    ("f", "Star/Un-star"),
                ]);
            }
            Page::Playlists => {
                binds.extend([
                    ("Enter", "Play"),
                    ("←/→", "Focus"),
                    ("e", "Add"),
                    ("n", "Add next"),
                    ("s", "Shuffle play"),
                ]);
            }
            Page::Radio => {
                binds.extend([
                    ("Enter", "Play"),
                    ("Space", "Play/Pause"),
                    ("Ctrl+R", "Refresh"),
                ]);
            }
            Page::Server => {
                binds.extend([
                    ("Tab", "Next field"),
                    ("Enter", "Test/Save"),
                    ("Ctrl+R", "Refresh"),
                ]);
            }
            Page::Settings => {
                binds.extend([("←/→/Enter", "Change theme")]);
            }
        }

        binds
    }
}

impl Widget for Footer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let chunks = Layout::horizontal([Constraint::Min(40), Constraint::Length(30)]).split(area);
        let left = chunks[0];
        let right = chunks[1];

        // Right side: repeat, volume, sample rate (always on first row)
        let mut status_parts = Vec::new();
        if self.repeat_mode != RepeatMode::Off {
            status_parts.push(format!("Repeat: {}", self.repeat_mode.label()));
        }
        status_parts.push(format!("Vol: {}%", self.volume));
        if let Some(rate) = self.sample_rate {
            let khz = rate as f64 / 1000.0;
            let rate_str = if khz == khz.floor() {
                format!("{}kHz", khz as u32)
            } else {
                format!("{:.1}kHz", khz)
            };
            status_parts.push(rate_str);
        }
        let status = status_parts.join(" | ");
        let max_width = right.width as usize;
        if !status.is_empty() {
            let display_status = if status.len() > max_width {
                &status[status.len() - max_width..]
            } else {
                &status
            };
            let x = right.x + right.width.saturating_sub(display_status.len() as u16);
            buf.set_string(
                x,
                right.y,
                display_status,
                Style::default().fg(self.colors.success),
            );
        }

        // Left side: keybinds or notification
        if let Some(notif) = self.notification {
            let style = if notif.is_error {
                Style::default().fg(self.colors.error)
            } else {
                Style::default().fg(self.colors.success)
            };
            buf.set_string(left.x, left.y, &notif.message, style);
            return;
        }

        let binds = self.keybinds();
        if binds.is_empty() {
            return;
        }

        let build_line = |slice: &[(&'static str, &'static str)]| {
            let mut spans = Vec::new();
            for (i, (key, desc)) in slice.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(
                        " │ ",
                        Style::default().fg(self.colors.secondary),
                    ));
                }
                spans.push(Span::styled(*key, Style::default().fg(self.colors.accent)));
                spans.push(Span::raw(":"));
                spans.push(Span::styled(*desc, Style::default().fg(self.colors.muted)));
            }
            Line::from(spans)
        };

        // If only one row available, render all keybinds on one line (truncated)
        if area.height < 2 {
            let line = build_line(&binds);
            buf.set_line(left.x, left.y, &line, left.width);
            return;
        }

        // Split keybinds across two rows
        let mid = (binds.len() + 1) / 2;
        let line1 = build_line(&binds[..mid]);
        let line2 = build_line(&binds[mid..]);
        buf.set_line(left.x, left.y, &line1, left.width);
        buf.set_line(left.x, left.y + 1, &line2, left.width);
    }
}
