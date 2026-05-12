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

        // Give the right side a minimum of 18 so status text is never unreadable.
        let chunks = Layout::horizontal([Constraint::Min(0), Constraint::Min(18)]).split(area);
        let left = chunks[0];
        let right = chunks[1];

        // Right side: repeat, volume, sample rate (always on first row)
        let status = self.format_status(right.width as usize);
        if !status.is_empty() {
            let x = right.x + right.width.saturating_sub(status.len() as u16);
            buf.set_string(
                x,
                right.y,
                &status,
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

impl Footer<'_> {
    /// Build the status string, abbreviating parts to fit the given width.
    fn format_status(&self, max_width: usize) -> String {
        let rate_str = self.sample_rate.map(|rate| {
            let khz = rate as f64 / 1000.0;
            if khz == khz.floor() {
                format!("{}kHz", khz as u32)
            } else {
                format!("{:.1}kHz", khz)
            }
        });

        // Full form: "Repeat: All | Vol: 100% | 44.1kHz"
        let mut parts = Vec::new();
        if self.repeat_mode != RepeatMode::Off {
            parts.push(format!("Repeat: {}", self.repeat_mode.label()));
        }
        parts.push(format!("Vol: {}%", self.volume));
        if let Some(ref r) = rate_str {
            parts.push(r.clone());
        }
        let full = parts.join(" | ");
        if full.len() <= max_width {
            return full;
        }

        // Short form: "R:All | V:100% | 44.1kHz"
        parts.clear();
        if self.repeat_mode != RepeatMode::Off {
            parts.push(format!("R:{}", self.repeat_mode.label()));
        }
        parts.push(format!("V:{}%", self.volume));
        if let Some(ref r) = rate_str {
            parts.push(r.clone());
        }
        let short = parts.join(" | ");
        if short.len() <= max_width {
            return short;
        }

        // Minimal form: just volume (and sample rate if it fits)
        let mut minimal = format!("V:{}%", self.volume);
        if let Some(ref r) = rate_str {
            let with_rate = format!("{} | {}", minimal, r);
            if with_rate.len() <= max_width {
                minimal = with_rate;
            }
        }
        minimal
    }
}
