# cramp
## Custom rust audio / music player

The defining feature of cramp is that it's mine. I am only too glad to accept
feature suggestions and pull requests, but the whole reason I made this is to
have unorthodox features in my own player. If there's a feature I don't need, I
probably won't implement it.

However, I am (again) happy to accept suggestions or help. If this somehow
reaches anyone else, I will expand the scope of the project to make it more
public-friendly. Until then, expect bugs, inconveniences, and incongruities.

#### Features:
 - a decent gui + strictly-searchable song list
 - opening and playing a folder of music or M3U playlist
 - basic shuffle
 - almost all the [MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/) (think playerctl) basic spec
    - seeking is currently not available, see [this issue](https://github.com/Kyllingene/cramp/issues/1)
 - super basic [M3U](https://en.wikipedia.org/wiki/M3U) support; only `#EXTINF` is supported for now
 - a 100-song history
 - two custom M3U tags:
   - `#EXTNEXT:<next-song-uri>` to denote a "back-to-back" song
     - song A always selects song B as the next song
   - `#EXTNOSHUFFLE` to stop a song from being automatically played in a shuffled queue

#### Roadmap:
 - implement `Seek` and `Metadata`
 - a more robust interface
    - playback position + seeking
    - fully-featured and ergonomic playlist management
      - possibly validating MPRIS `Playlists`
 - more customized features
 - a CLI
 - implement `TrackList`
 - configuration?
    - maybe plugin support?

#### Non-goals:
 - a tui (or at least not a good one)
 - a web build
 - streaming support (use pulseaudio)
 - playing web resources (sorry, download it or find another player)
 - playing audio streams

#### Known bugs:
 - the MPRIS implementation doesn't emit the `PropertiesChanged` signal

If you find a bug that isn't listed, [open an issue](https://github.com/kyllingene/issue/new).
