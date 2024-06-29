use slotmap::SlotMap;

use std::collections::VecDeque;

use crate::song::Song;
use crate::Message;

slotmap::new_key_type! { pub struct Key; }

#[derive(Default)]
pub struct Queue {
    pub songs: SlotMap<Key, Song>,

    playlist: VecDeque<Key>,
    user_queue: VecDeque<Key>,
    next: Option<Key>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
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

                let id = self.songs.insert(Song::new(path).next(next.take()).no_shuffle(no_shuffle));

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
