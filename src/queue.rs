use std::collections::VecDeque;
use std::fmt::Display;
use std::fs::{read_dir, read_to_string, File};
use std::io;
use std::mem;
use std::ops::AddAssign;
use std::path::Path;

use rand::{seq::SliceRandom, thread_rng};
use rodio::{OutputStream, Sink};

use crate::song::{LoadedSong, Song};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoopMode {
    #[default]
    None,
    Track,
    Playlist,
}

impl AddAssign<usize> for LoopMode {
    fn add_assign(&mut self, rhs: usize) {
        let modes = [Self::None, Self::Track, Self::Playlist];
        let i = match self {
            LoopMode::None => 0,
            LoopMode::Track => 1,
            LoopMode::Playlist => 2,
        };

        *self = modes[(i + rhs) % 3];
    }
}

impl From<&str> for LoopMode {
    fn from(s: &str) -> Self {
        match s {
            "Track" => Self::Track,
            "Playlist" => Self::Playlist,
            _ => Self::None,
        }
    }
}

impl From<String> for LoopMode {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl Display for LoopMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct Queue {
    // all the songs in the playlist
    pub songs: Vec<Song>,

    // the currently playing song
    pub current: Option<Song>,
    // the next song
    pub next: Option<LoadedSong>,

    // the queue of songs (this is what gets shuffled)
    pub queue: VecDeque<Song>,
    // the user's queued songs
    pub user_queue: VecDeque<Song>,

    // the queue of past songs (up to 100)
    pub past: Vec<Song>,

    volume: f32,
    pub shuffle: bool,

    pub loop_mode: LoopMode,

    // set to true to signal an exit
    pub quit: bool,

    // the audio output;
    // `_stream` must be kept in scope for `sink` to work
    sink: Sink,
    _stream: OutputStream,
}

impl Default for Queue {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.pause();

        Self {
            songs: Vec::new(),

            current: None,
            next: None,

            queue: VecDeque::new(),
            user_queue: VecDeque::new(),
            past: Vec::with_capacity(100),

            volume: 1.0,
            shuffle: false,

            quit: false,

            loop_mode: LoopMode::Playlist,

            sink,
            _stream,
        }
    }
}

unsafe impl Sync for Queue {}
unsafe impl Send for Queue {}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    // load a song from a playlist file
    // supports the M3U #EXTINF flag, as well as
    // a custom `#EXTNEXT:<next-song>` flag, and
    // a custom `#EXTNOSHUFFLE` flag
    pub fn load<P: AsRef<Path>>(file: P) -> Self {
        let mut queue = Queue::new();

        let data = read_to_string(file).unwrap();

        let mut songs = Vec::new();

        let mut length = None;
        let mut name = None;
        let mut next = None;
        let mut noshuffle = false;

        for line in data.lines() {
            if line.is_empty() {
                continue;
            }

            if let Some(line) = line.strip_prefix("#EXTINF:") {
                let bits = line.split(',').collect::<Vec<&str>>();

                // this gets overridden if song.open()
                // can get the length of the song
                if let Ok(l) = bits[0].parse::<u128>() {
                    length = Some(l * 1000000);
                }

                // this is because m3u names are often just full paths,
                // so make sure it's not a path before setting it as the name
                let givenname = bits.into_iter().skip(1).collect::<Vec<&str>>().join(",");
                if Path::new(&givenname)
                    .parent()
                    .map_or(true, |p| p.as_os_str().is_empty())
                {
                    name = Some(givenname);
                }
            } else if let Some(line) = line.strip_prefix("#EXTNEXT:") {
                next = Some(line.to_string());
            } else if line.strip_prefix("#EXTNOSHUFFLE").is_some() {
                noshuffle = true;
            } else if !line.starts_with('#') {
                songs.push(
                    Song::new(line.to_string(), name.take(), next.take(), length)
                        .noshuffle(mem::take(&mut noshuffle)),
                );
            }
        }

        queue.songs = songs;
        queue
    }

    // load all the music in a directory (doesn't check extensions)
    pub fn load_dir<P: AsRef<Path>>(path: P) -> Self {
        Self {
            songs: Self::load_dir_entry(path),
            ..Default::default()
        }
    }

    // recursively load all the music in a directory (doesn't check extensions)
    fn load_dir_entry<P: AsRef<Path>>(dir: P) -> Vec<Song> {
        let mut songs = Vec::new();
        for entry in read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_dir() {
                songs.append(&mut Self::load_dir_entry(path));
            } else {
                songs.push(Song::new(path.display(), None, None, None));
            }
        }

        songs
    }

    // save the current playlist to a file, saving
    // all M3U flags that cramp supports
    pub fn save_playlist<P: AsRef<Path>>(&self, path: P) {
        use std::io::Write;
        let mut file = File::create(path).unwrap();

        writeln!(file, "#EXTM3U").unwrap();
        for song in &self.songs {
            if let Some(name) = &song.save_name {
                writeln!(file, "#EXTINF:{},{name}", song.length.unwrap_or(0)).unwrap();
            }

            if let Some(next) = &song.next {
                writeln!(file, "#EXTNEXT:{next}").unwrap();
            }

            if song.noshuffle {
                writeln!(file, "#EXTNOSHUFFLE").unwrap();
            }

            writeln!(file, "{}", song.file).unwrap();
        }
    }

    pub fn play(&mut self) {
        if let Some(song) = &mut self.current {
            self.sink.append(song.open().unwrap());
        }

        self.sink.play();
    }

    pub fn pause(&mut self) {
        self.sink.pause();
    }

    pub fn play_pause(&mut self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        self.sink.set_volume(volume);
    }

    pub fn stop(&mut self) {
        self.pause();
        self.sink.stop();
    }

    pub fn remove(&mut self, id: u64) -> Option<Song> {
        if let Some((i, _)) = self
            .songs
            .iter()
            .enumerate()
            .find(|(_, song)| song.id == id)
        {
            Some(self.songs.remove(i))
        } else {
            None
        }
    }

    pub fn next(&mut self) {
        self.sink.stop();

        if self.loop_mode == LoopMode::Track {
            self.play();
            return;
        }

        if let Some(song) = self.current.take() {
            self.past.push(song);
            self.past.reverse();
            self.past.truncate(100);
            self.past.reverse();
        }

        if self.queue.is_empty() && self.loop_mode == LoopMode::Playlist {
            self.queue = self.songs.clone().into();

            if self.shuffle {
                self.shuffle();
            }
        }

        if let Some(song) = self.next.take() {
            self.current = Some(song.song);
            self.sink.append(song.data);
        }

        // if the new song has a preferred next song, set next to that
        self.next = if let Some(Some(next)) = &self.current.as_ref().map(|s| s.next.clone()) {
            if let Some(song) = self.songs.iter_mut().find(|s| &s.file == next) {
                song.into()
            } else if self.shuffle {
                if let Some((i, _)) = self.queue.iter().enumerate().find(|(_, s)| !s.noshuffle) {
                    self.queue.remove(i).unwrap().into()
                } else if let Some(song) = self.queue.pop_front() {
                    song.into()
                } else {
                    None
                }
            } else if let Some(song) = self.queue.pop_front() {
                song.into()
            } else {
                None
            }
        } else if let Some(song) = self.user_queue.pop_front() {
            song.into()
        } else if self.shuffle {
            if let Some((i, _)) = self
                .queue
                .iter()
                .enumerate()
                .rev()
                .find(|(_, s)| !s.noshuffle)
            {
                self.queue.remove(i).unwrap().into()
            } else if let Some(song) = self.queue.pop_front() {
                song.into()
            } else {
                None
            }
        } else if let Some(song) = self.queue.pop_front() {
            song.into()
        } else {
            None
        };
    }

    // stops the current song, appends next to queue,
    // sets next to current song, sets current to past.pop
    pub fn last(&mut self) {
        self.sink.stop();

        if self.loop_mode == LoopMode::Track {
            self.play();
            return;
        }

        if let Some(song) = self.current.take() {
            if let Some(song) = self.next.take() {
                self.queue.push_back(song.song);
            }

            self.next = song.into();
        }

        self.current = self.past.pop();
        if let Some(song) = &mut self.current {
            self.sink.append(song.open().unwrap());
        }
    }

    // shuffles self.queue (NOT self.songs)
    pub fn shuffle(&mut self) {
        if !self.shuffle {
            self.queue.make_contiguous().shuffle(&mut thread_rng());
        } else {
            // try to preserve location in the playlist when unshuffling (except for next)
            if let Some(song) = &self.current {
                let index = self
                    .songs
                    .iter()
                    .enumerate()
                    .find(|(_, s)| song.file == s.file)
                    .map(|s| s.0);

                if let Some(index) = index {
                    self.queue = VecDeque::from(self.songs[index..].to_vec());
                } else {
                    self.queue = self.songs.clone().into();
                }
            } else {
                self.queue = self.songs.clone().into();
            }
        }

        self.shuffle = !self.shuffle;
    }

    pub fn queue(&mut self, song: Song) {
        self.user_queue.push_back(song);
    }

    pub fn queue_all(&mut self) {
        self.queue = self.songs.clone().into();
    }

    // returns true if nothing is currently playing or paused
    pub fn empty(&self) -> bool {
        self.sink.empty()
    }

    pub fn volume(&self) -> f32 {
        self.sink.volume()
    }

    pub fn paused(&self) -> bool {
        self.sink.is_paused()
    }

    // add a file to the playlist, and play it
    pub fn add_file<P: AsRef<Path>>(&mut self, file: P) -> io::Result<()> {
        let mut file: String = file.as_ref().display().to_string();
        if let Some(f) = file.strip_prefix("file://") {
            file = String::from("/") + f;
        }

        self.songs.push(Song::new(file, None, None, None));
        self.current = self.songs.last().cloned();

        println!("{:?}", self.current);

        self.sink.stop();
        self.sink.append(self.current.as_mut().unwrap().open()?);
        self.play();

        Ok(())
    }

    pub(crate) fn speed(&self) -> f32 {
        self.sink.speed()
    }

    pub(crate) fn set_speed(&self, speed: f32) {
        self.sink.set_speed(speed);
    }
}
