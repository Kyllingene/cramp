use circular_buffer::CircularBuffer;
use rand::{seq::SliceRandom, thread_rng};
use slotmap::SlotMap;

use std::collections::VecDeque;
use std::fs;
use std::path::Path;

use crate::song::Song;
use crate::Message;

slotmap::new_key_type! { pub struct Key; }

#[derive(Default)]
pub struct Queue {
    songs: SlotMap<Key, Song>,

    /// The list of all songs to play.
    playlist: VecDeque<Key>,

    /// The end point (exclusive) of the user queue in `playlist`.
    user_queue: usize,

    /// Whether or not the next song was specifically requested.
    explicit_next: bool,

    /// The currently playing song, if any.
    current: Option<Key>,

    /// The songs played to completion.
    ///
    /// Keeps up to 32 entries.
    history: CircularBuffer<32, Key>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: Key) -> &Song {
        self.songs.get(id).expect("given invalid key")
    }

    pub fn songs(&self) -> impl Iterator<Item = (Key, &Song)> {
        self.songs
            .iter()
            .filter_map(|(key, song)| song.user_added.then_some((key, song)))
    }

    pub fn current(&self) -> Option<&Song> {
        self.current.map(|id| self.get(id))
    }

    pub fn current_id(&self) -> Option<Key> {
        self.current
    }

    pub fn next(&self) -> Option<&Song> {
        self.playlist.front().map(|id| self.get(*id))
    }

    pub fn playlist(&self) -> impl Iterator<Item = &Song> + '_ {
        self.playlist.iter().map(|id| self.get(*id)).skip(1)
    }

    pub fn advance(&mut self) {
        if let Some(id) = self.current.take() {
            self.history.push_back(id);
        }

        if self.playlist.is_empty() {
            // TODO: make this an option
            self.queue_all();
            self.shuffle();
        }

        if let Some(id) = self.playlist.pop_front() {
            self.current = Some(id);

            if let Some(path) = &self.get(id).next {
                // FIXME: deduplicate please
                let mut song = Song::new(path);
                song.user_added = false;
                let id = self.songs.insert(song);
                self.playlist.push_front(id);
                self.explicit_next = true;
            } else {
                self.explicit_next = false;
                self.user_queue = self.user_queue.saturating_sub(1);
            }
        }

        if self.playlist.is_empty() {
            // TODO: make this an option
            self.queue_all();
            self.shuffle();
        }
    }

    pub fn previous(&mut self) {
        if let Some(id) = self.current.take() {
            self.playlist.push_front(id);
        }

        if let Some(id) = self.history.pop_back() {
            self.current = Some(id);
        }
    }

    pub fn shuffle(&mut self) {
        self.playlist.make_contiguous();
        self.playlist.as_mut_slices().0[self.user_queue..].shuffle(&mut thread_rng());
    }

    pub fn play(&mut self, id: Key) -> &Song {
        if let Some(id) = self.current.take() {
            self.history.push_back(id);
        }

        self.current = Some(id);
        self.get(id)
    }

    pub fn set_next(&mut self, id: Key) -> &Song {
        if self.explicit_next {
            self.playlist[0] = id;
        } else {
            self.playlist.push_front(id);
            self.explicit_next = true;
            self.user_queue += 1;
        }

        self.get(id)
    }

    pub fn add_song(&mut self, song: Song) -> Key {
        self.songs.insert(song)
    }

    pub fn queue(&mut self, id: Key, user_queue: bool) {
        if user_queue {
            self.playlist.insert(self.user_queue, id);
            self.user_queue += 1;
        } else {
            self.playlist.push_back(id);
        }
    }

    pub fn queue_all(&mut self) {
        self.playlist = self
            .songs
            .iter()
            .filter_map(|(id, song)| (song.user_added && !song.no_shuffle).then_some(id))
            .collect();

        self.explicit_next = false;
        self.user_queue = 0;
    }

    pub fn load(&mut self, path: impl AsRef<Path>) -> impl Iterator<Item = Message> {
        let path = path.as_ref();
        if path.is_dir() {
            self.load_dir(path)
        } else if path.is_file() {
            let Some(ext) = path.extension().map(|s| s.to_string_lossy()) else {
                return Vec::new().into_iter();
            };

            match ext.as_ref() {
                "m3u" | "m3u4" => self.load_playlist(path),
                _ => {
                    let song = Song::new(path);
                    let id = self.add_song(song);
                    self.playlist.push_back(id);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
        .into_iter()
    }

    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> Vec<Message> {
        let read = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) => {
                return vec![Message::new(format!(
                    "failed to enumerate {}: {e}",
                    dir.as_ref().display()
                ))];
            }
        };

        let mut messages = Vec::new();
        for entry in read {
            match entry {
                Ok(entry) => {
                    messages.extend(self.load(entry.path()));
                }
                Err(e) => {
                    messages.push(Message::new(format!(
                        "failed to read {}: {e}",
                        dir.as_ref().display()
                    )));
                }
            }
        }

        messages
    }

    pub fn load_playlist(&mut self, path: impl AsRef<Path>) -> Vec<Message> {
        let mut messages = Vec::new();

        let playlist = match std::fs::read_to_string(path) {
            Ok(p) => p,
            Err(e) => {
                messages.push(Message::new(format!("failed to load playlist: {e}")));
                return messages;
            }
        };

        let home = dirs::home_dir().unwrap_or_else(|| {
            messages.push(Message::stc(
                "failed to get home directory! defaulting to cwd",
            ));
            "./".into()
        });

        let mut next = None;
        let mut no_shuffle = false;
        for line in playlist.lines() {
            if let Some(ext) = line.strip_prefix('#') {
                if ext.trim() == "EXTNOSHUFFLE" {
                    no_shuffle = true;
                } else if let Some(path) = ext.strip_prefix("EXTNEXT:") {
                    next = Some(path);
                }
            } else {
                let path = if let Some(rest) = line.strip_prefix("~/") {
                    let mut path = home.clone();
                    path.push(rest);
                    path
                } else {
                    line.into()
                };

                let id = self
                    .songs
                    .insert(Song::new(path).next(next.take()).no_shuffle(no_shuffle));

                if !no_shuffle {
                    self.playlist.push_back(id);
                }

                no_shuffle = false;
            }
        }

        messages.push(Message::new(format!(
            "loaded {} songs from playlist",
            self.songs.len()
        )));

        messages
    }
}
