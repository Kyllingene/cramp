use std::{sync::mpsc::Receiver, time::Duration};

use cod::{
    prelude::*,
    read::{KeyCode, KeyEvent, KeyModifiers},
};

use crate::{player::Player, queue::Queue, Message};

pub enum Event {
    Exit,
    Shuffle,

    Next,
    Prev,

    PlayPause,

    SeekRight,
    SeekLeft,
}

#[derive(Default)]
pub struct Ui {
    messages: Vec<Message>,
}

impl Ui {
    pub fn new() -> Self {
        term::secondary_screen();
        term::enable_raw_mode();
        clear::all();
        cod::flush();

        Self::default()
    }

    pub fn event(&mut self, key: KeyEvent, rx: &Receiver<KeyEvent>) -> Option<Event> {
        let ev = match key.code {
            KeyCode::Char('q') => {
                self.clear();
                let (w, _) = term::size_or();
                draw_centered(3, "do you want to exit?", None, w, true);
                draw_centered(4, "press y to exit", None, w, false);
                self.flush();

                if rx.recv_timeout(Duration::from_secs(5)).ok()?.code == KeyCode::Char('y') {
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
            _ => None,
        };

        if ev.is_some() {
            self.messages.clear();
        }

        ev
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

        for (i, song) in queue.playlist().enumerate().take(h as usize - 11) {
            draw_centered(i as u32 + 10, &song.name, None, w, false);
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
        goto::home();
        cod::flush();
    }
}

fn draw_centered(y: u32, msg: &str, pre: Option<&str>, w: u32, bold: bool) {
    let mid = (w / 2)
        .saturating_sub((msg.len() as u32 + pre.map(|s| s.len() as u32).unwrap_or(0)) / 2);
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
        term::primary_screen();
        term::disable_raw_mode();
    }
}
