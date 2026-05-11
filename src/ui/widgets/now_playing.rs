//! Now playing display widget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::app::state::NowPlaying;
use crate::ui::theme::ThemeColors;

/// Now playing panel widget
pub struct NowPlayingWidget<'a> {
    now_playing: &'a NowPlaying,
    focused: bool,
    colors: ThemeColors,
}

impl<'a> NowPlayingWidget<'a> {
    pub fn new(now_playing: &'a NowPlaying, colors: ThemeColors) -> Self {
        Self {
            now_playing,
            focused: false,
            colors,
        }
    }

    #[allow(dead_code)]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for NowPlayingWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Need at least 6 rows for full display
        if area.height < 4 || area.width < 20 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Now Playing ")
            .border_style(if self.focused {
                Style::default().fg(self.colors.border_focused)
            } else {
                Style::default().fg(self.colors.border_unfocused)
            });

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 {
            return;
        }

        // Check if something is playing
        if self.now_playing.song.is_none() && self.now_playing.radio_station.is_none() {
            let no_track = Paragraph::new("No track playing")
                .style(Style::default().fg(self.colors.muted))
                .alignment(Alignment::Center);
            no_track.render(inner, buf);
            return;
        }

        if let Some(station) = self.now_playing.radio_station.as_ref() {
            let title = self
                .now_playing
                .radio_title
                .as_deref()
                .unwrap_or(&station.name);
            let artist = self
                .now_playing
                .radio_artist
                .as_deref()
                .unwrap_or("Internet Radio");
            let subtitle = station.home_page_url.as_deref().unwrap_or(&station.name);
            let quality = build_quality_string(self.now_playing);

            if inner.height >= 5 {
                let chunks = Layout::vertical([
                    Constraint::Length(1), // Artist
                    Constraint::Length(1), // Station/subtitle
                    Constraint::Length(1), // Title
                    Constraint::Length(1), // Quality
                    Constraint::Length(1), // Elapsed
                ])
                .split(inner);

                Paragraph::new(Line::from(vec![Span::styled(
                    artist,
                    Style::default().fg(self.colors.artist),
                )]))
                .alignment(Alignment::Center)
                .render(chunks[0], buf);

                Paragraph::new(Line::from(vec![Span::styled(
                    subtitle,
                    Style::default().fg(self.colors.album),
                )]))
                .alignment(Alignment::Center)
                .render(chunks[1], buf);

                Paragraph::new(Line::from(vec![Span::styled(
                    title,
                    Style::default()
                        .fg(self.colors.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                )]))
                .alignment(Alignment::Center)
                .render(chunks[2], buf);

                if !quality.is_empty() {
                    Paragraph::new(Line::from(vec![Span::styled(
                        quality,
                        Style::default().fg(self.colors.muted),
                    )]))
                    .alignment(Alignment::Center)
                    .render(chunks[3], buf);
                }

                render_elapsed(chunks[4], buf, self.now_playing, &self.colors);
            } else if inner.height >= 3 {
                let chunks = Layout::vertical([
                    Constraint::Length(1), // Title - Artist
                    Constraint::Length(1), // Station / Quality
                    Constraint::Length(1), // Elapsed
                ])
                .split(inner);

                let line1 = Line::from(vec![
                    Span::styled(
                        title,
                        Style::default()
                            .fg(self.colors.highlight_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" - ", Style::default().fg(self.colors.muted)),
                    Span::styled(artist, Style::default().fg(self.colors.artist)),
                ]);
                Paragraph::new(line1)
                    .alignment(Alignment::Center)
                    .render(chunks[0], buf);

                let line2_text = if quality.is_empty() {
                    subtitle
                } else {
                    &quality
                };
                Paragraph::new(Line::from(vec![Span::styled(
                    line2_text,
                    Style::default().fg(self.colors.album),
                )]))
                .alignment(Alignment::Center)
                .render(chunks[1], buf);

                render_elapsed(chunks[2], buf, self.now_playing, &self.colors);
            } else {
                let chunks =
                    Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);
                Paragraph::new(Line::from(vec![Span::styled(
                    title,
                    Style::default().fg(self.colors.highlight_fg),
                )]))
                .alignment(Alignment::Center)
                .render(chunks[0], buf);
                render_elapsed(chunks[1], buf, self.now_playing, &self.colors);
            }

            return;
        }

        let Some(song) = self.now_playing.song.as_ref() else {
            return;
        };

        // Build centered lines like Go version:
        // Line 1: Artist (green)
        // Line 2: Album (purple/magenta)
        // Line 3: Title (white, bold)
        // Line 4: Quality info (gray)
        // Line 5: Progress bar

        let artist = song.artist.clone().unwrap_or_default();
        let album = song.album.clone().unwrap_or_default();
        let title = song.title.clone();

        // Build quality string
        let quality = build_quality_string(self.now_playing);

        // Layout based on available height
        if inner.height >= 5 {
            // Full layout with separate lines
            let chunks = Layout::vertical([
                Constraint::Length(1), // Artist
                Constraint::Length(1), // Album
                Constraint::Length(1), // Title
                Constraint::Length(1), // Quality
                Constraint::Length(1), // Progress
            ])
            .split(inner);

            // Artist line (centered, artist color)
            let artist_line = Line::from(vec![Span::styled(
                &artist,
                Style::default().fg(self.colors.artist),
            )]);
            Paragraph::new(artist_line)
                .alignment(Alignment::Center)
                .render(chunks[0], buf);

            // Album line (centered, album color)
            let album_line = Line::from(vec![Span::styled(
                &album,
                Style::default().fg(self.colors.album),
            )]);
            Paragraph::new(album_line)
                .alignment(Alignment::Center)
                .render(chunks[1], buf);

            // Title line (centered, bold)
            let title_line = Line::from(vec![Span::styled(
                &title,
                Style::default()
                    .fg(self.colors.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            )]);
            Paragraph::new(title_line)
                .alignment(Alignment::Center)
                .render(chunks[2], buf);

            // Quality line (centered, muted)
            if !quality.is_empty() {
                let quality_line = Line::from(vec![Span::styled(
                    &quality,
                    Style::default().fg(self.colors.muted),
                )]);
                Paragraph::new(quality_line)
                    .alignment(Alignment::Center)
                    .render(chunks[3], buf);
            }

            // Progress bar
            render_progress_bar(
                chunks[4],
                buf,
                self.now_playing.progress_percent(),
                &self.now_playing.format_position(),
                &self.now_playing.format_duration(),
                &self.colors,
            );
        } else if inner.height >= 3 {
            // Compact layout
            let chunks = Layout::vertical([
                Constraint::Length(1), // Artist - Title
                Constraint::Length(1), // Album / Quality
                Constraint::Length(1), // Progress
            ])
            .split(inner);

            // Combined artist - title line
            let line1 = Line::from(vec![
                Span::styled(
                    &title,
                    Style::default()
                        .fg(self.colors.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" - ", Style::default().fg(self.colors.muted)),
                Span::styled(&artist, Style::default().fg(self.colors.artist)),
            ]);
            Paragraph::new(line1)
                .alignment(Alignment::Center)
                .render(chunks[0], buf);

            // Album line
            let line2 = Line::from(vec![Span::styled(
                &album,
                Style::default().fg(self.colors.album),
            )]);
            Paragraph::new(line2)
                .alignment(Alignment::Center)
                .render(chunks[1], buf);

            // Progress bar
            render_progress_bar(
                chunks[2],
                buf,
                self.now_playing.progress_percent(),
                &self.now_playing.format_position(),
                &self.now_playing.format_duration(),
                &self.colors,
            );
        } else {
            // Minimal layout
            let chunks = Layout::vertical([
                Constraint::Length(1), // Title
                Constraint::Length(1), // Progress
            ])
            .split(inner);

            let line1 = Line::from(vec![Span::styled(
                &title,
                Style::default().fg(self.colors.highlight_fg),
            )]);
            Paragraph::new(line1)
                .alignment(Alignment::Center)
                .render(chunks[0], buf);

            render_progress_bar(
                chunks[1],
                buf,
                self.now_playing.progress_percent(),
                &self.now_playing.format_position(),
                &self.now_playing.format_duration(),
                &self.colors,
            );
        }
    }
}

fn build_quality_string(now_playing: &NowPlaying) -> String {
    let mut quality_parts = Vec::new();
    if let Some(ref fmt) = now_playing.format {
        quality_parts.push(fmt.to_string().to_uppercase());
    }
    if let Some(bits) = now_playing.bit_depth {
        quality_parts.push(format!("{}-bit", bits));
    }
    if let Some(rate) = now_playing.sample_rate {
        let khz = rate as f64 / 1000.0;
        if khz == khz.floor() {
            quality_parts.push(format!("{}kHz", khz as u32));
        } else {
            quality_parts.push(format!("{:.1}kHz", khz));
        }
    }
    if let Some(ref channels) = now_playing.channels {
        quality_parts.push(channels.to_string());
    }
    quality_parts.join(" │ ")
}

fn render_elapsed(area: Rect, buf: &mut Buffer, now_playing: &NowPlaying, colors: &ThemeColors) {
    if area.width < 5 {
        return;
    }

    let elapsed = now_playing.format_position();
    let text = format!("Live │ {}", elapsed);
    let start_x = area.x + (area.width.saturating_sub(text.len() as u16)) / 2;
    buf.set_string(start_x, area.y, text, Style::default().fg(colors.muted));
}

/// Render a simple progress bar
fn render_progress_bar(
    area: Rect,
    buf: &mut Buffer,
    progress: f64,
    pos: &str,
    dur: &str,
    colors: &ThemeColors,
) {
    if area.width < 15 {
        return;
    }

    // Format: "00:00 / 00:00  [════════════────────]"
    let time_str = format!("{} / {}", pos, dur);
    let time_width = time_str.len() as u16;

    // Calculate positions - center the whole thing
    let bar_width = area.width.saturating_sub(time_width + 3); // 2 spaces + some padding
    let total_width = time_width + 2 + bar_width;
    let start_x = area.x + (area.width.saturating_sub(total_width)) / 2;

    // Draw time string
    buf.set_string(
        start_x,
        area.y,
        &time_str,
        Style::default().fg(colors.highlight_fg),
    );

    // Draw progress bar
    let bar_start = start_x + time_width + 2;
    if bar_width > 0 {
        let filled = (bar_width as f64 * progress) as u16;

        // Draw filled portion (success color like Go version)
        for x in bar_start..(bar_start + filled) {
            buf[(x, area.y)]
                .set_char('━')
                .set_style(Style::default().fg(colors.success));
        }

        // Draw empty portion
        for x in (bar_start + filled)..(bar_start + bar_width) {
            buf[(x, area.y)]
                .set_char('─')
                .set_style(Style::default().fg(colors.muted));
        }
    }
}
