//! Progress bar widget with seek support

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

/// A horizontal progress bar with time display
#[allow(dead_code)]
pub struct ProgressBar<'a> {
    /// Progress value (0.0 to 1.0)
    progress: f64,
    /// Current position formatted
    position_text: &'a str,
    /// Total duration formatted
    duration_text: &'a str,
    /// Filled portion style
    filled_style: Style,
    /// Empty portion style
    empty_style: Style,
    /// Text style
    text_style: Style,
}

#[allow(dead_code)]
impl<'a> ProgressBar<'a> {
    pub fn new(progress: f64, position_text: &'a str, duration_text: &'a str) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            position_text,
            duration_text,
            filled_style: Style::default().bg(Color::Blue),
            empty_style: Style::default().bg(Color::DarkGray),
            text_style: Style::default().fg(Color::White),
        }
    }

    pub fn filled_style(mut self, style: Style) -> Self {
        self.filled_style = style;
        self
    }

    pub fn empty_style(mut self, style: Style) -> Self {
        self.empty_style = style;
        self
    }

    pub fn text_style(mut self, style: Style) -> Self {
        self.text_style = style;
        self
    }

    /// Calculate position from x coordinate within the bar area
    pub fn position_from_x(area: Rect, x: u16) -> Option<f64> {
        // Account for time text margins
        let bar_start = area.x + 8; // "00:00 " prefix
        let bar_end = area.x + area.width - 8; // " 00:00" suffix

        if x >= bar_start && x < bar_end {
            let bar_width = bar_end - bar_start;
            let relative_x = x - bar_start;
            Some(relative_x as f64 / bar_width as f64)
        } else {
            None
        }
    }
}

impl Widget for ProgressBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 1 {
            return;
        }

        // Format: "00:00 [==========----------] 00:00"
        let pos_width = self.position_text.len();
        let dur_width = self.duration_text.len();

        // Draw position text
        buf.set_string(area.x, area.y, self.position_text, self.text_style);

        // Draw duration text
        let dur_x = area.x + area.width - dur_width as u16;
        buf.set_string(dur_x, area.y, self.duration_text, self.text_style);

        // Calculate bar area
        let bar_x = area.x + pos_width as u16 + 1;
        let bar_width = area
            .width
            .saturating_sub((pos_width + dur_width + 2) as u16);

        if bar_width > 0 {
            let filled_width = (bar_width as f64 * self.progress) as u16;

            // Draw filled portion
            for x in bar_x..(bar_x + filled_width) {
                buf[(x, area.y)].set_char('━').set_style(self.filled_style);
            }

            // Draw empty portion
            for x in (bar_x + filled_width)..(bar_x + bar_width) {
                buf[(x, area.y)].set_char('─').set_style(self.empty_style);
            }
        }
    }
}

/// Vertical gauge (for volume, etc.)
#[allow(dead_code)]
pub struct VerticalBar {
    /// Value (0.0 to 1.0)
    value: f64,
    /// Filled style
    filled_style: Style,
    /// Empty style
    empty_style: Style,
}

#[allow(dead_code)]
impl VerticalBar {
    pub fn new(value: f64) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            filled_style: Style::default().bg(Color::Blue),
            empty_style: Style::default().bg(Color::DarkGray),
        }
    }

    pub fn filled_style(mut self, style: Style) -> Self {
        self.filled_style = style;
        self
    }

    pub fn empty_style(mut self, style: Style) -> Self {
        self.empty_style = style;
        self
    }
}

impl Widget for VerticalBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }

        let filled_height = (area.height as f64 * self.value) as u16;
        let empty_start = area.y + area.height - filled_height;

        // Draw empty portion (top)
        for y in area.y..empty_start {
            for x in area.x..(area.x + area.width) {
                buf[(x, y)].set_char('░').set_style(self.empty_style);
            }
        }

        // Draw filled portion (bottom)
        for y in empty_start..(area.y + area.height) {
            for x in area.x..(area.x + area.width) {
                buf[(x, y)].set_char('█').set_style(self.filled_style);
            }
        }
    }
}
