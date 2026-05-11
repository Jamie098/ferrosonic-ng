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
                    ("/", "Search"),
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

        // Left side: keybinds or notification
        if let Some(notif) = self.notification {
            let style = if notif.is_error {
                Style::default().fg(self.colors.error)
            } else {
                Style::default().fg(self.colors.success)
            };
            buf.set_string(chunks[0].x, chunks[0].y, &notif.message, style);
        } else {
            // Keybind hints
            let binds = self.keybinds();
            let mut spans = Vec::new();

            for (i, (key, desc)) in binds.iter().enumerate() {
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

            let line = Line::from(spans);
            buf.set_line(chunks[0].x, chunks[0].y, &line, chunks[0].width);
        }

        // Right side: repeat, volume, sample rate
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
        if !status.is_empty() {
            let x = chunks[1].x + chunks[1].width.saturating_sub(status.len() as u16);
            buf.set_string(
                x,
                chunks[1].y,
                &status,
                Style::default().fg(self.colors.success),
            );
        }
    }
}
