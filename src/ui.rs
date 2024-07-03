use std::sync::mpsc::{channel, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// use async_std::channel::{unbounded, Receiver};
use async_std::sync::Mutex;
use cod::{
    prelude::*,
    read::{KeyCode, KeyEvent, KeyModifiers},
};

use crate::player::Player;
use crate::queue::{Key, Queue};
use crate::song::Song;
use crate::Message;

pub enum Event {
    Exit,
    Shuffle,

    PlayPause,
    Next,
    Prev,

    PlayNow(Key),
    PlayNext(Key),
    Queue(Key),

    SeekRight,
    SeekLeft,
}

pub struct Ui {
    messages: Vec<Message>,

    songs: usize,
    song_idx: usize,

    search: Option<String>,
    search_idx: usize,

    rx: Mutex<Receiver<KeyEvent>>,
    _handle: JoinHandle<()>,
}

impl Ui {
    pub async fn new() -> Self {
        term::secondary_screen();
        term::enable_raw_mode();
        clear::all();
        cod::flush();

        let (tx, rx) = channel();
        let _handle = thread::spawn(move || loop {
            if let Some(key) = cod::read::key() {
                tx.send(key).unwrap();
            }
        });

        Self {
            messages: Vec::with_capacity(4),

            songs: 0,
            song_idx: 0,

            search: None,
            search_idx: 0,

            rx: Mutex::new(rx),
            _handle,
        }
    }

    pub async fn event(&mut self, queue: &Queue) -> Option<Event> {
        let key = self
            .rx
            .lock()
            .await
            .recv_timeout(Duration::from_millis(10))
            .ok()?;
        self.messages.clear();

        'search: {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                break 'search;
            }

            if let Some(search) = &mut self.search {
                match key.code {
                    KeyCode::Char(ch) => {
                        search.insert(self.search_idx, ch.to_ascii_lowercase());
                        self.search_idx += 1;
                    }
                    KeyCode::Backspace => {
                        if self.search_idx > 0 {
                            self.search_idx -= 1;
                            search.remove(self.search_idx);
                        } else if search.is_empty() {
                            self.search = None;
                            self.search_idx = 0;
                        }
                    }
                    KeyCode::Right => self.search_idx += 1,
                    KeyCode::Left => self.search_idx = self.search_idx.saturating_sub(1),
                    KeyCode::Esc => {
                        self.search = None;
                        self.search_idx = 0;
                    }
                    _ => break 'search,
                }

                // TODO: adjust selection, don't reset it
                self.songs = 0;
                self.song_idx = 0;

                return None;
            }
        }

        match key.code {
            KeyCode::Char('q') => {
                self.clear();
                let (w, _) = term::size_or();
                draw_centered(3, "do you want to exit?", None, w, true);
                draw_centered(4, "press y to exit", None, w, false);
                self.flush();

                if self
                    .rx
                    .lock()
                    .await
                    .recv_timeout(Duration::from_secs(2))
                    .ok()?
                    .code
                    == KeyCode::Char('y')
                {
                    Some(Event::Exit)
                } else {
                    None
                }
            }
            KeyCode::Char('s') => Some(Event::Shuffle),
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    Some(Event::SeekRight)
                } else {
                    Some(Event::Next)
                }
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    Some(Event::SeekLeft)
                } else {
                    Some(Event::Prev)
                }
            }
            KeyCode::Char(' ') => Some(Event::PlayPause),

            KeyCode::Char('/') => {
                if self.search.is_none() {
                    self.search = Some(String::with_capacity(16));
                }
                None
            }

            KeyCode::Down => {
                self.song_idx += 1;
                if self.song_idx > 5 {
                    self.songs += 1;
                }
                None
            }
            KeyCode::Up => {
                self.song_idx = self.song_idx.saturating_sub(1);
                if self.song_idx < self.songs {
                    self.songs = self.song_idx;
                }
                None
            }

            KeyCode::Enter => filter_songs(queue.songs(), &self.search)
                .nth(self.song_idx)
                .map(|(id, _)| Event::PlayNow(id)),
            KeyCode::Char('n') => filter_songs(queue.songs(), &self.search)
                .nth(self.song_idx)
                .map(|(id, _)| Event::PlayNext(id)),
            KeyCode::Char('a') => filter_songs(queue.songs(), &self.search)
                .nth(self.song_idx)
                .map(|(id, _)| Event::Queue(id)),

            _ => None,
        }
    }

    pub fn draw(&mut self, queue: &Queue, player: &Player) {
        let (w, h) = term::size_or();

        let current = if let Some(song) = queue.current() {
            &song.name
        } else {
            "<no song playing>"
        };
        draw_centered(2, current, Some("now: "), w, true);

        let next = if let Some(song) = queue.next() {
            &song.name
        } else {
            "<no song is next>"
        };
        draw_centered(3, next, Some("next: "), w, true);

        draw_centered(
            5,
            if player.playing() {
                "playing"
            } else {
                "paused"
            },
            None,
            w,
            true,
        );

        if let Some((elapsed, total)) = player.time_info() {
            draw_centered(6, &fmt_time(elapsed, total), None, w, false);
        }

        draw_centered(8, "queued", None, w, true);
        for (i, song) in queue.playlist().enumerate().take(5) {
            draw_centered(i as u32 + 10, &song.name, None, w, false);
        }

        if let Some(search) = &self.search {
            draw_centered(16, search, Some("search: "), w, true);
        } else {
            draw_centered(16, "songs", None, w, true);
        }

        let selected = self.song_idx - self.songs;
        for (i, song) in filter_songs(queue.songs(), &self.search)
            .map(|(_, song)| song)
            .skip(self.songs)
            .enumerate()
            .take(h as usize - 19)
        {
            let pre = if i == selected { Some("> ") } else { None };

            draw_centered(i as u32 + 18, &song.name, pre, w, i == selected);
        }

        for (i, message) in self.messages.iter().enumerate() {
            draw_centered(h - i as u32, message, None, w, true);
        }
    }

    pub fn clear(&mut self) {
        clear::all();
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn flush(&self) {
        if self.search.is_some() {
            let (w, _) = term::size_or();
            goto::pos(
                (w / 2) + (self.search_idx as u32 + 8 + (self.search_idx as u32 % 2)) / 2,
                16,
            );
        } else {
            goto::home();
        }

        cod::flush();
    }

    pub fn exit(&self) {
        term::primary_screen();
        term::disable_raw_mode();
    }
}

fn draw_centered(y: u32, msg: &str, pre: Option<&str>, w: u32, bold: bool) {
    let mid =
        (w / 2).saturating_sub((msg.len() as u32 + pre.map(|s| s.len() as u32).unwrap_or(0)) / 2);
    goto::pos(mid, y);

    if let Some(pre) = pre {
        print!("{pre}");
    }

    if bold {
        style::with::bold(|| print!("{msg}"));
    } else {
        print!("{msg}");
    }
}

fn filter_songs<'a>(
    songs: impl Iterator<Item = (Key, &'a Song)>,
    search: &Option<String>,
) -> impl Iterator<Item = (Key, &'a Song)> {
    let filter = if let Some(search) = search {
        let search = search.clone();
        Box::new(move |(_, song): &(Key, &Song)| {
            let mut path = song.path.display().to_string();
            path.make_ascii_lowercase();
            path.contains(&search)
        }) as Box<dyn Fn(&(Key, &Song)) -> bool>
    } else {
        Box::new(|_: &(Key, &Song)| true) as Box<_>
    };

    songs.filter(filter)
}

fn fmt_time(elapsed: f64, total: f64) -> String {
    let raw_secs = elapsed;
    let esecs = raw_secs as u32 % 60;

    let raw_mins = raw_secs / 60.0;
    let emins = raw_mins as u32 % 60;

    let raw_hrs = raw_mins / 60.0;
    let ehrs = raw_hrs as u32 % 60;

    let raw_secs = total;
    let tsecs = raw_secs as u32 % 60;

    let raw_mins = raw_secs / 60.0;
    let tmins = raw_mins as u32 % 60;

    let raw_hrs = raw_mins / 60.0;
    let thrs = raw_hrs as u32 % 60;

    if ehrs > 0 || thrs > 0 {
        format!("{ehrs}:{emins:02}:{esecs:02} / {thrs}:{tmins:02}:{tsecs:02}")
    } else {
        format!("{emins:02}:{esecs:02} / {tmins:02}:{tsecs:02}")
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        self.exit();
    }
}
