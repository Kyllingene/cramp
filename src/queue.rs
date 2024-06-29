use circular_buffer::CircularBuffer;
use rand::{seq::SliceRandom, thread_rng};
use slotmap::SlotMap;

use std::collections::VecDeque;

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

    pub fn current(&self) -> Option<&Song> {
        self.current.map(|id| self.get(id))
    }

    pub fn next(&self) -> Option<&Song> {
        self.playlist.front().map(|id| self.get(*id))
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

    pub fn set_next(&mut self, id: Key) {
        if self.explicit_next {
            self.playlist[0] = id;
        } else {
            self.playlist.push_front(id);
            self.explicit_next = true;
            self.user_queue += 1;
        }
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

    pub fn load(&mut self, playlist: &str) -> impl Iterator<Item = Message> {
        let mut messages = [None, None];

        let home = dirs::home_dir().unwrap_or_else(|| {
            messages[0] = Some(Message::stc(
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

                // TODO: should I log ignored flags?
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

        messages[1] = Some(Message::new(format!(
            "loaded {} songs from playlist",
            self.songs.len()
        )));

        messages.into_iter().flatten()
    }
}
