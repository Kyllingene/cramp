use std::fmt;
use std::ops::Deref;

mod player;
mod queue;
mod song;

use player::Player;
use queue::Queue;

pub enum Message {
    Static(&'static str),
    Dynamic(String),
}

impl Message {
    pub fn new(s: impl ToString) -> Self {
        Self::Dynamic(s.to_string())
    }

    pub fn stc(s: &'static str) -> Self {
        Self::Static(s)
    }
}

impl Deref for Message {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Dynamic(s) => s,
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            match self {
                Self::Static(s) => *s,
                Self::Dynamic(s) => s,
            }
        )
    }
}

fn main() {
    let mut queue = Queue::new();
    let mut player = Player::new();

    let playlist = std::fs::read_to_string("/home/kyllingene/Music/sleep.m3u").unwrap();
    let _ = queue.load(&playlist);

    for song in queue.songs.values() {
        player.play(song).unwrap();
        println!("playing {}", song.name);
        // std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
