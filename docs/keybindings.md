# Keybindings Reference

Ferrosonic is fully keyboard-driven. Vim-style `j`/`k` navigation is available alongside arrow keys throughout the application.

## Global

| Key | Action |
|---|---|
| `q` | Quit |
| `p` / `Space` | Toggle play/pause |
| `l` | Next track |
| `h` | Previous track |
| `shift + h` | Seek backward 5 seconds |
| `shift + l` | Seek forward 5 seconds |
| `Ctrl+R` | Refresh data from server |
| `t` | Cycle to next theme |
| `F1` | Browse page |
| `F2` | Artists page |
| `F3` | Queue page |
| `F4` | Playlists page |
| `F5` | Radio page |
| `F6` | Lyrics page | 
| `F7` | Server configuration page |
| `F8` | Settings page |

## Songs Page (F1)

| Key | Action |
|---|---|
| `Tab` | Switch focus between song options and song list |
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Enter` | Play selected song (queues all visible songs and starts from selection) |

The Songs page has two modes selectable from the options pane: **Starred** (your favourited songs) and **Random** (a random selection from the server).

## Artists Page (F2)

| Key | Action |
|---|---|
| `/` | Filter artists by name |
| `Esc` | Clear filter |
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Left` / `Right` | Switch focus between tree and song list |
| `Enter` | Expand/collapse artist, or play album/song |
| `Backspace` | Return to tree from song list |
| `e` | Add selected item to end of queue |
| `n` | Add selected item as next in queue |
| `s` | Shuffle play all songs by the selected artist or album |

## Queue Page (F3)

| Key | Action |
|---|---|
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Enter` | Play selected song |
| `d` | Remove selected song from queue |
| `J` (Shift+J) | Move selected song down |
| `K` (Shift+K) | Move selected song up |
| `s` | Shuffle queue (current song stays in place) |
| `c` | Clear played history (remove songs before current) |

## Playlists Page (F4)

| Key | Action |
|---|---|
| `Tab` / `Left` / `Right` | Switch focus between playlists and songs |
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Enter` | Load playlist songs or play selected song |
| `e` | Add selected item to end of queue |
| `n` | Add selected song as next in queue |
| `s` | Shuffle play all songs in selected playlist |

## Radio Page (F5)

| Key | Action |
|---|---|
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Enter` | Play selected station |
| `Space` | Play selected station, or toggle pause if it is already current |
| `Ctrl+R` | Refresh all data, including radio stations |

The Radio page lists internet radio stations from servers that support the Subsonic `getInternetRadioStations` endpoint, such as Navidrome.

## Lyrics Page (F6)

| Key | Action |
|---|---|
| `Up` / `k` | Move lyrics up |
| `Down` / `j` | Move lyrics down |
| `Top` / `g` | Move to first line of the lyrics currently loaded |
| `Increase line spacing` / `]` | Adds spacing equivalent to one line to the lyrics |
| `Decrease line spacing` / `[` | Removes spacing equivalent to one line to the lyrics | 
| `Page Up` /`Crtl + b`  | Page up in the lyrics tab | 
| `Page Down` /`Crtl + f`  | Page down in the lyrics tab | 
| `Auto-scroll` / `a` | Resume auto-scroll |

The Lyrics page support the following Subsonic endpoints: `getLyricsBySongId` (Preferred, modern), and `getLyrics` (Fallback, legacy version). Both implementations were tested through Navidrome.
Note that in order to increase the size(lyrics font) the terminal itself needs to be zoomed(Ctrl and +, or Ctrl and mouse wheel up).
The Auto-scroll command only works when either the scroll options or Page Up/Down have been used previously, then pressing `a` will go back into the portion of the lyrics that is currently highlighted
Auto-scroll option is default option for .lrc and embedded lyrics, plain text files without a timestamp won't have the highlighted synced line nor the auto-scroll feature working. That's what the current manual scroll keybindings try to solve.

## Server Page (F7)

| Key | Action |
|---|---|
| `Tab` | Move between fields |
| `Enter` | Test connection or Save configuration |
| `Backspace` | Delete character in text field |

## Settings Page (F8)

| Key | Action |
|---|---|
| `Up` / `Down` | Move between settings |
| `Left` | Previous option |
| `Right` / `Enter` | Next option |

Settings include theme selection and cava visualizer toggle. Changes are saved automatically.

