# cramp
## cool(?) rust audio / music player - INDEV

#### currently implemented:
 - a basic gui + searchable (no fzf, sorry) song list
 - opening and playing a folder of music
 - basic shuffle
 - almost all of the [MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/) (think playerctl) basic spec, with some features only boilerplate-implemented (Seek, Position, etc.), except `Metadata`
 - super basic [M3U](https://en.wikipedia.org/wiki/M3U) support; only one tag (`#EXTINF:<length>,<name>`) supported for now
 - a custom "M3U" tag (starts with `#EXTNEXT:<next-song-uri>`) to denote a "back-to-back" song (see below)
 - a 100-song history

#### roadmap:
 - the MPRIS `Metadata` property
 - a more robust interface
    - ability to browse/create/save playlists
    - ability to search through a playlist
    - ability to set "back-to-back" songs from the tui
        - example: whenever you play song A, regardless of shuffle/queue, always plays song B next
 - more customized features
    - maybe plugin support?

#### non-goals:
 - a tui (or at least not a good one)
 - a web build (how would that work?)
 - streaming support (use pulseaudio)
 - a daemon (like mpd)
 - playing web resources (sorry, download it or find another music player)
 - more complex playlist support (probably)

#### known bugs:
 - an inexplicable alsa-related crash, that's top priority at the moment
    - possibly has something to do with long songs and timeouts
 - you have to send several MPRIS messages to get cramp to start responding
    - various other MPRIS-related bugs, sorry, that's not top priority
 - probably loads more, be patient (or better, help out)