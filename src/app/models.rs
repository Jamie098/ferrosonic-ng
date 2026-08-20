use strum_macros::{Display, EnumIter};

#[derive(Display, EnumIter, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrowseTab {
    #[default]
    Songs,
    Albums,
}

#[derive(Display, EnumIter, Clone, Debug, PartialEq)]
pub enum SongOption {
    All,
    Starred,
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub timestamp: Option<std::time::Duration>,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedLyrics {
    pub lines: Vec<LyricLine>,
    pub is_synced: bool,
}

impl ParsedLyrics {
    pub fn new(raw: &str) -> Self {
        let mut lines = Vec::new();
        let mut is_synced = false;

        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with('[') {
                if let Some(end_idx) = line.find(']') {
                    let timestamp_str = &line[1..end_idx];
                    if let Some(duration) = parse_timestamp(timestamp_str) {
                        is_synced = true;
                        let text = line[end_idx + 1..].trim().to_string();
                        lines.push(LyricLine {
                            timestamp: Some(duration),
                            text,
                        });
                        continue;
                    }
                }
            }

            lines.push(LyricLine {
                timestamp: None,
                text: line.to_string(),
            });
        }

        // Sort synced lines by timestamp just in case
        if is_synced {
            lines.sort_by(|a, b| {
                a.timestamp
                    .unwrap_or_default()
                    .cmp(&b.timestamp.unwrap_or_default())
            });
        }

        Self { lines, is_synced }
    }

    pub fn current_index(&self, position: std::time::Duration) -> Option<usize> {
        if !self.is_synced || self.lines.is_empty() {
            return None;
        }

        let mut current = 0;
        for (i, line) in self.lines.iter().enumerate() {
            if let Some(ts) = line.timestamp {
                if ts <= position {
                    current = i;
                } else {
                    break;
                }
            }
        }
        Some(current)
    }
}

fn parse_timestamp(s: &str) -> Option<std::time::Duration> {
    // Format: [mm:ss.xx] or [mm:ss:xx] or [mm:ss]
    let parts: Vec<&str> = s.split(|c| c == ':' || c == '.').collect();
    if parts.len() < 2 {
        return None;
    }

    let minutes: u64 = parts[0].parse().ok()?;
    let seconds: u64 = parts[1].parse().ok()?;
    let mut millis: u64 = 0;

    if parts.len() >= 3 {
        let sub = parts[2];
        if sub.len() == 2 {
            // hundredths
            millis = sub.parse::<u64>().ok()? * 10;
        } else if sub.len() == 3 {
            // milliseconds
            millis = sub.parse::<u64>().ok()?;
        }
    }

    Some(std::time::Duration::from_secs(minutes * 60 + seconds) + std::time::Duration::from_millis(millis))
}
