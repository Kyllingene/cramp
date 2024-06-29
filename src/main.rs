use std::ops::Deref;
use std::thread;
use std::time::Duration;
use std::{fmt, sync::mpsc::channel};

mod player;
mod queue;
mod song;
mod ui;

use player::Player;
use queue::Queue;
use ui::{Event, Ui};

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
    let mut ui = Ui::new();

    let playlist = std::fs::read_to_string("/home/kyllingene/Music/favorites.m3u").unwrap();
    for message in queue.load(&playlist) {
        ui.add_message(message);
    }

    queue.queue_all();
    queue.shuffle();

    let (tx, rx) = channel();
    let _handle = thread::spawn(move || loop {
        if let Some(key) = cod::read::key() {
            tx.send(key).unwrap();
        }
    });

    loop {
        if player.finished() && player.playing() {
            queue.advance();
            if let Some(song) = queue.current() {
                if let Err(e) = player.play(song) {
                    ui.add_message(Message::new(format!(
                        "failed to play song {}: {e}",
                        song.path.display()
                    )));
                }
                if let Some(song) = queue.next() {
                    if let Err(e) = player.load_next(song) {
                        ui.add_message(Message::new(format!(
                            "failed to load song {}: {e}",
                            song.path.display()
                        )));
                    }
                }
            }
        }

        ui.draw(&queue, &player);
        ui.flush();

        if let Ok(key) = rx.recv_timeout(Duration::from_secs(1)) {

            // FIXME: query pause information from Queue, not Player
            let paused = !player.playing();

            match ui.event(key, &rx) {
                Some(Event::Exit) => break,
                Some(Event::Shuffle) => queue.shuffle(),
                Some(Event::Next) => {
                    player.stop();
                    queue.advance();
                    if let Some(song) = queue.current() {
                        if let Err(e) = player.play(song) {
                            ui.add_message(Message::new(format!(
                                "failed to play song {}: {e}",
                                song.path.display()
                            )));
                        }
                        if paused {
                            player.pause();
                        }
                    }
                    if let Some(song) = queue.next() {
                        if let Err(e) = player.load_next(song) {
                            ui.add_message(Message::new(format!(
                                "failed to load song {}: {e}",
                                song.path.display()
                            )));
                        }
                    }
                }
                Some(Event::Prev) => {
                    player.stop();
                    queue.previous();
                    if let Some(song) = queue.current() {
                        if let Err(e) = player.play(song) {
                            ui.add_message(Message::new(format!(
                                "failed to play song {}: {e}",
                                song.path.display()
                            )));
                        }
                        if paused {
                            player.pause();
                        }
                    }
                    if let Some(song) = queue.next() {
                        if let Err(e) = player.load_next(song) {
                            ui.add_message(Message::new(format!(
                                "failed to load song {}: {e}",
                                song.path.display()
                            )));
                        }
                    }
                }
                Some(Event::PlayPause) => {
                    if player.playing() {
                        player.pause();
                    } else {
                        player.resume();
                    }
                }
                Some(Event::SeekRight) => player.seek_by(5.0),
                Some(Event::SeekLeft) => player.seek_by(-5.0),
                None => {}
            }
        }

        ui.clear();
    }
}
