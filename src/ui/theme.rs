//! Theme color definitions — file-based themes loaded from ~/.config/ferrosonic/themes/

use std::path::Path;

use ratatui::style::Color;
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::config::paths;

/// Color palette for a theme
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    /// Primary highlight color (focused elements, selected tabs)
    pub primary: Color,
    /// Secondary color (borders, less important elements)
    pub secondary: Color,
    /// Accent color (currently playing, important highlights)
    pub accent: Color,
    /// Artist names
    pub artist: Color,
    /// Album names
    pub album: Color,
    /// Song titles (default)
    pub song: Color,
    /// Muted text (track numbers, durations, hints)
    pub muted: Color,
    /// Selection/highlight background
    pub highlight_bg: Color,
    /// Text on highlighted background
    pub highlight_fg: Color,
    /// Success messages
    pub success: Color,
    /// Error messages
    pub error: Color,
    /// Playing indicator
    pub playing: Color,
    /// Played songs in queue
    pub played: Color,
    /// Border color (focused)
    pub border_focused: Color,
    /// Border color (unfocused)
    pub border_unfocused: Color,
}

/// A loaded theme: display name + colors + cava gradients
#[derive(Debug, Clone)]
pub struct ThemeData {
    /// Display name (e.g. "Catppuccin", "Default")
    pub name: String,
    /// UI colors
    pub colors: ThemeColors,
    /// Cava vertical gradient (8 hex strings)
    pub cava_gradient: [String; 8],
    /// Cava horizontal gradient (8 hex strings)
    pub cava_horizontal_gradient: [String; 8],
}

// ── TOML deserialization structs ──────────────────────────────────────────────

#[derive(Deserialize)]
struct ThemeFile {
    colors: ThemeFileColors,
    cava: Option<ThemeFileCava>,
}

#[derive(Deserialize)]
struct ThemeFileColors {
    primary: String,
    secondary: String,
    accent: String,
    artist: String,
    album: String,
    song: String,
    muted: String,
    highlight_bg: String,
    highlight_fg: String,
    success: String,
    error: String,
    playing: String,
    played: String,
    border_focused: String,
    border_unfocused: String,
}

#[derive(Deserialize)]
struct ThemeFileCava {
    gradient: Option<Vec<String>>,
    horizontal_gradient: Option<Vec<String>>,
}

// ── Hex color parsing ─────────────────────────────────────────────────────────

fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    warn!("Invalid hex color '{}', falling back to white", hex);
    Color::White
}

fn parse_gradient(values: &[String], fallback: &[&str; 8]) -> [String; 8] {
    let mut result: [String; 8] = std::array::from_fn(|i| fallback[i].to_string());
    for (i, v) in values.iter().enumerate().take(8) {
        result[i] = v.clone();
    }
    result
}

// ── ThemeData construction ────────────────────────────────────────────────────

impl ThemeData {
    fn from_file_content(name: &str, content: &str) -> Result<Self, String> {
        let file: ThemeFile =
            toml::from_str(content).map_err(|e| format!("Failed to parse theme '{}': {}", name, e))?;

        let c = &file.colors;
        let colors = ThemeColors {
            primary: hex_to_color(&c.primary),
            secondary: hex_to_color(&c.secondary),
            accent: hex_to_color(&c.accent),
            artist: hex_to_color(&c.artist),
            album: hex_to_color(&c.album),
            song: hex_to_color(&c.song),
            muted: hex_to_color(&c.muted),
            highlight_bg: hex_to_color(&c.highlight_bg),
            highlight_fg: hex_to_color(&c.highlight_fg),
            success: hex_to_color(&c.success),
            error: hex_to_color(&c.error),
            playing: hex_to_color(&c.playing),
            played: hex_to_color(&c.played),
            border_focused: hex_to_color(&c.border_focused),
            border_unfocused: hex_to_color(&c.border_unfocused),
        };

        let default_g: [&str; 8] = [
            "#59cc33", "#cccc33", "#cc8033", "#cc5533",
            "#cc3333", "#bb1111", "#990000", "#990000",
        ];
        let default_h: [&str; 8] = [
            "#c45161", "#e094a0", "#f2b6c0", "#f2dde1",
            "#cbc7d8", "#8db7d2", "#5e62a9", "#434279",
        ];

        let cava = file.cava.as_ref();
        let cava_gradient = match cava.and_then(|c| c.gradient.as_ref()) {
            Some(g) => parse_gradient(g, &default_g),
            None => std::array::from_fn(|i| default_g[i].to_string()),
        };
        let cava_horizontal_gradient = match cava.and_then(|c| c.horizontal_gradient.as_ref()) {
            Some(h) => parse_gradient(h, &default_h),
            None => std::array::from_fn(|i| default_h[i].to_string()),
        };

        Ok(ThemeData {
            name: name.to_string(),
            colors,
            cava_gradient,
            cava_horizontal_gradient,
        })
    }

    /// The hardcoded Default theme
    pub fn default_theme() -> Self {
        ThemeData {
            name: "Default".to_string(),
            colors: ThemeColors {
                primary: Color::Cyan,
                secondary: Color::DarkGray,
                accent: Color::Yellow,
                artist: Color::LightGreen,
                album: Color::Magenta,
                song: Color::Magenta,
                muted: Color::Gray,
                highlight_bg: Color::Rgb(102, 51, 153),
                highlight_fg: Color::White,
                success: Color::Green,
                error: Color::Red,
                playing: Color::LightGreen,
                played: Color::Red,
                border_focused: Color::Cyan,
                border_unfocused: Color::DarkGray,
            },
            cava_gradient: [
                "#59cc33".into(), "#cccc33".into(), "#cc8033".into(), "#cc5533".into(),
                "#cc3333".into(), "#bb1111".into(), "#990000".into(), "#990000".into(),
            ],
            cava_horizontal_gradient: [
                "#c45161".into(), "#e094a0".into(), "#f2b6c0".into(), "#f2dde1".into(),
                "#cbc7d8".into(), "#8db7d2".into(), "#5e62a9".into(), "#434279".into(),
            ],
        }
    }
}

// ── Loading ───────────────────────────────────────────────────────────────────

/// Load all themes: Default (hardcoded) + TOML files from themes dir (sorted alphabetically)
pub fn load_themes() -> Vec<ThemeData> {
    let mut themes = vec![ThemeData::default_theme()];

    if let Some(dir) = paths::themes_dir() {
        if dir.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |ext| ext == "toml")
                })
                .collect();
            entries.sort_by_key(|e| e.file_name());

            for entry in entries {
                let path = entry.path();
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                // Capitalize first letter for display name
                let name = titlecase_filename(stem);

                match std::fs::read_to_string(&path) {
                    Ok(content) => match ThemeData::from_file_content(&name, &content) {
                        Ok(theme) => {
                            info!("Loaded theme '{}' from {}", name, path.display());
                            themes.push(theme);
                        }
                        Err(e) => error!("{}", e),
                    },
                    Err(e) => error!("Failed to read {}: {}", path.display(), e),
                }
            }
        }
    }

    themes
}

/// Convert a filename stem like "tokyo-night" or "rose_pine" to "Tokyo Night" or "Rose Pine"
fn titlecase_filename(s: &str) -> String {
    s.split(|c: char| c == '-' || c == '_')
        .filter(|w| !w.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Seeding built-in themes ───────────────────────────────────────────────────

/// Write the built-in themes as TOML files into the given directory.
/// Only writes files that don't already exist.
pub fn seed_default_themes(dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        error!("Failed to create themes directory: {}", e);
        return;
    }

    for (filename, content) in BUILTIN_THEMES {
        let path = dir.join(filename);
        if !path.exists() {
            if let Err(e) = std::fs::write(&path, content) {
                error!("Failed to write theme {}: {}", filename, e);
            } else {
                info!("Seeded theme file: {}", filename);
            }
        }
    }
}

const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("monokai.toml", r##"[colors]
primary = "#a6e22e"
secondary = "#75715e"
accent = "#fd971f"
artist = "#a6e22e"
album = "#f92672"
song = "#e6db74"
muted = "#75715e"
highlight_bg = "#49483e"
highlight_fg = "#f8f8f2"
success = "#a6e22e"
error = "#f92672"
playing = "#fd971f"
played = "#75715e"
border_focused = "#a6e22e"
border_unfocused = "#49483e"

[cava]
gradient = ["#a6e22e", "#e6db74", "#fd971f", "#fd971f", "#f92672", "#f92672", "#ae81ff", "#ae81ff"]
horizontal_gradient = ["#f92672", "#f92672", "#fd971f", "#e6db74", "#e6db74", "#a6e22e", "#a6e22e", "#66d9ef"]
"##),

    ("dracula.toml", r##"[colors]
primary = "#bd93f9"
secondary = "#6272a4"
accent = "#ffb86c"
artist = "#50fa7b"
album = "#ff79c6"
song = "#8be9fd"
muted = "#6272a4"
highlight_bg = "#44475a"
highlight_fg = "#f8f8f2"
success = "#50fa7b"
error = "#ff5555"
playing = "#ffb86c"
played = "#6272a4"
border_focused = "#bd93f9"
border_unfocused = "#44475a"

[cava]
gradient = ["#50fa7b", "#8be9fd", "#8be9fd", "#bd93f9", "#bd93f9", "#ff79c6", "#ff5555", "#ff5555"]
horizontal_gradient = ["#ff79c6", "#ff79c6", "#bd93f9", "#bd93f9", "#8be9fd", "#8be9fd", "#50fa7b", "#50fa7b"]
"##),

    ("nord.toml", r##"[colors]
primary = "#88c0d0"
secondary = "#4c566a"
accent = "#ebcb8b"
artist = "#a3be8c"
album = "#b48ead"
song = "#88c0d0"
muted = "#4c566a"
highlight_bg = "#434c5e"
highlight_fg = "#eceff4"
success = "#a3be8c"
error = "#bf616a"
playing = "#ebcb8b"
played = "#4c566a"
border_focused = "#88c0d0"
border_unfocused = "#3b4252"

[cava]
gradient = ["#a3be8c", "#88c0d0", "#88c0d0", "#81a1c1", "#81a1c1", "#5e81ac", "#b48ead", "#b48ead"]
horizontal_gradient = ["#bf616a", "#d08770", "#ebcb8b", "#a3be8c", "#88c0d0", "#81a1c1", "#5e81ac", "#b48ead"]
"##),

    ("gruvbox.toml", r##"[colors]
primary = "#d79921"
secondary = "#928374"
accent = "#fe8019"
artist = "#b8bb26"
album = "#d3869b"
song = "#83a598"
muted = "#928374"
highlight_bg = "#504945"
highlight_fg = "#ebdbb2"
success = "#b8bb26"
error = "#fb4934"
playing = "#fe8019"
played = "#928374"
border_focused = "#d79921"
border_unfocused = "#3c3836"

[cava]
gradient = ["#b8bb26", "#d79921", "#d79921", "#fe8019", "#fe8019", "#fb4934", "#cc241d", "#cc241d"]
horizontal_gradient = ["#cc241d", "#fb4934", "#fe8019", "#d79921", "#b8bb26", "#689d6a", "#458588", "#83a598"]
"##),

    ("catppuccin.toml", r##"[colors]
primary = "#89b4fa"
secondary = "#585b70"
accent = "#f9e2af"
artist = "#a6e3a1"
album = "#f5c2e7"
song = "#94e2d5"
muted = "#6c7086"
highlight_bg = "#45475a"
highlight_fg = "#cdd6f4"
success = "#a6e3a1"
error = "#f38ba8"
playing = "#f9e2af"
played = "#6c7086"
border_focused = "#89b4fa"
border_unfocused = "#45475a"

[cava]
gradient = ["#a6e3a1", "#94e2d5", "#89dceb", "#74c7ec", "#cba6f7", "#f5c2e7", "#f38ba8", "#f38ba8"]
horizontal_gradient = ["#f38ba8", "#eba0ac", "#fab387", "#f9e2af", "#a6e3a1", "#94e2d5", "#89b4fa", "#cba6f7"]
"##),

    ("solarized.toml", r##"[colors]
primary = "#268bd2"
secondary = "#586e75"
accent = "#b58900"
artist = "#859900"
album = "#d33682"
song = "#2aa198"
muted = "#586e75"
highlight_bg = "#073642"
highlight_fg = "#eee8d5"
success = "#859900"
error = "#dc322f"
playing = "#b58900"
played = "#586e75"
border_focused = "#268bd2"
border_unfocused = "#073642"

[cava]
gradient = ["#859900", "#b58900", "#b58900", "#cb4b16", "#cb4b16", "#dc322f", "#d33682", "#6c71c4"]
horizontal_gradient = ["#dc322f", "#cb4b16", "#b58900", "#859900", "#2aa198", "#268bd2", "#6c71c4", "#d33682"]
"##),

    ("tokyo-night.toml", r##"[colors]
primary = "#7aa2f7"
secondary = "#3d59a1"
accent = "#e0af68"
artist = "#9ece6a"
album = "#bb9af7"
song = "#7dcfff"
muted = "#565f89"
highlight_bg = "#292e42"
highlight_fg = "#c0caf5"
success = "#9ece6a"
error = "#f7768e"
playing = "#e0af68"
played = "#565f89"
border_focused = "#7aa2f7"
border_unfocused = "#292e42"

[cava]
gradient = ["#9ece6a", "#e0af68", "#e0af68", "#ff9e64", "#ff9e64", "#f7768e", "#bb9af7", "#bb9af7"]
horizontal_gradient = ["#f7768e", "#ff9e64", "#e0af68", "#9ece6a", "#73daca", "#7dcfff", "#7aa2f7", "#bb9af7"]
"##),

    ("rose-pine.toml", r##"[colors]
primary = "#c4a7e7"
secondary = "#6e6a86"
accent = "#f6c177"
artist = "#9ccfd8"
album = "#ebbcba"
song = "#31748f"
muted = "#6e6a86"
highlight_bg = "#393552"
highlight_fg = "#e0def4"
success = "#9ccfd8"
error = "#eb6f92"
playing = "#f6c177"
played = "#6e6a86"
border_focused = "#c4a7e7"
border_unfocused = "#393552"

[cava]
gradient = ["#31748f", "#9ccfd8", "#c4a7e7", "#c4a7e7", "#ebbcba", "#ebbcba", "#eb6f92", "#eb6f92"]
horizontal_gradient = ["#eb6f92", "#ebbcba", "#f6c177", "#f6c177", "#9ccfd8", "#c4a7e7", "#31748f", "#31748f"]
"##),

    ("everforest.toml", r##"[colors]
primary = "#a7c080"
secondary = "#859289"
accent = "#dbbc7f"
artist = "#83c092"
album = "#d699b6"
song = "#7fbbb3"
muted = "#859289"
highlight_bg = "#505851"
highlight_fg = "#d3c6aa"
success = "#a7c080"
error = "#e67e80"
playing = "#dbbc7f"
played = "#859289"
border_focused = "#a7c080"
border_unfocused = "#505851"

[cava]
gradient = ["#a7c080", "#dbbc7f", "#dbbc7f", "#e69875", "#e69875", "#e67e80", "#d699b6", "#d699b6"]
horizontal_gradient = ["#e67e80", "#e69875", "#dbbc7f", "#a7c080", "#83c092", "#7fbbb3", "#d699b6", "#d699b6"]
"##),

    ("kanagawa.toml", r##"[colors]
primary = "#7e9cd8"
secondary = "#54546d"
accent = "#e6c384"
artist = "#98bb6c"
album = "#957fb8"
song = "#7fb4ca"
muted = "#727169"
highlight_bg = "#363646"
highlight_fg = "#dcd7ba"
success = "#98bb6c"
error = "#ff5d62"
playing = "#e6c384"
played = "#727169"
border_focused = "#7e9cd8"
border_unfocused = "#363646"

[cava]
gradient = ["#98bb6c", "#e6c384", "#e6c384", "#ffa066", "#ffa066", "#ff5d62", "#957fb8", "#957fb8"]
horizontal_gradient = ["#ff5d62", "#ffa066", "#e6c384", "#98bb6c", "#7fb4ca", "#7e9cd8", "#957fb8", "#938aa9"]
"##),

    ("one-dark.toml", r##"[colors]
primary = "#61afef"
secondary = "#5c6370"
accent = "#e5c07b"
artist = "#98c379"
album = "#c678dd"
song = "#56b6c2"
muted = "#5c6370"
highlight_bg = "#3e4451"
highlight_fg = "#abb2bf"
success = "#98c379"
error = "#e06c75"
playing = "#e5c07b"
played = "#5c6370"
border_focused = "#61afef"
border_unfocused = "#3e4451"

[cava]
gradient = ["#98c379", "#e5c07b", "#e5c07b", "#d19a66", "#d19a66", "#e06c75", "#c678dd", "#c678dd"]
horizontal_gradient = ["#e06c75", "#d19a66", "#e5c07b", "#98c379", "#56b6c2", "#61afef", "#c678dd", "#c678dd"]
"##),

    ("ayu-dark.toml", r##"[colors]
primary = "#59c2ff"
secondary = "#6b788a"
accent = "#e6b450"
artist = "#aad94c"
album = "#d2a6ff"
song = "#95e6cb"
muted = "#6b788a"
highlight_bg = "#2f3846"
highlight_fg = "#bfc7d5"
success = "#aad94c"
error = "#f07178"
playing = "#e6b450"
played = "#6b788a"
border_focused = "#59c2ff"
border_unfocused = "#2f3846"

[cava]
gradient = ["#aad94c", "#e6b450", "#e6b450", "#ff8f40", "#ff8f40", "#f07178", "#d2a6ff", "#d2a6ff"]
horizontal_gradient = ["#f07178", "#ff8f40", "#e6b450", "#aad94c", "#95e6cb", "#59c2ff", "#d2a6ff", "#d2a6ff"]
"##),
];
