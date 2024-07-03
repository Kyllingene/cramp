# cramp
## Custom rust audio / music player

### UPDATE: as of 2.0, cramp is a whole new (TUI) application!

Cramp is my personal music player, so I try to keep it up to my (esoteric)
standards. Most recently this includes rewriting it from scratch to support more
capabilities, and reduce CPU usage drastically.

## Features

- Extremely (almost unfairly) opinionated (e.g. no unshuffle)
- A low-cost, sleek TUI interface
- A competent MPRIS interface (but no volume, rate, or shuffle)
- Support for *very basic* playlists (list of songs), only recognizes...
- Two custom `m3u` tags:
    - `#EXTNOSHUFFLE`: don't shuffle in this song when shuffling the playlist
    - `#EXTNEXT:<path>`: full path to a song to force-play after the current song
- A separate "user queue" and playlist
- A searchable song list
- A 32-song history

## Non-features

I omit the following because I don't need them:

- A GUI
- A competent playlist interface
- Linear playback (always shuffled)
- Volume or rate control
- Windows support
- Probably a lot more features you'd think were basic

## Controls

### Basics
- `q`: exit the player (confirms first)
- `space`: play/pause
- `Right`: skip to the next song
- `Left`: return to the previous song

### Seeking
- `Ctrl-Right`: seek 5 seconds forward in the song
- `Ctrl-Left`: seek 5 seconds backward in the song

### Selection
- `Up`/`Down`: go up/down in the song list (bottom)
- `Enter`: play the selected song now
- `n`: play the selected song next
- `a`: append the current song to the "user queue"
    - User queue goes after the next song, but before the rest of the playlist
- `/`: enter "search" mode:
    - Type to filter songs *by path*, e.g. `foo` would match
      `/home/music/foo/bar.mp3`
    - `Esc` to exit search
    - `Ctrl-<key>` to pass a letter through, e.g. `Ctrl-n` to set selected as
      next
- `s`: shuffle the playlist (not including user-queued songs)
