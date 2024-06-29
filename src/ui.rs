use std::{sync::mpsc::Receiver, time::Duration};

use cod::{
    prelude::*,
    read::{KeyCode, KeyEvent},
};

use crate::{player::Player, queue::Queue, Message};

pub enum Event {
    Exit,
    Shuffle,

    Next,
    Prev,

    PlayPause,
}

#[derive(Default)]
pub struct Ui {
    messages: u32,
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
        match key.code {
            KeyCode::Char('q') => {
                self.clear();
                let (w, _) = term::size_or();
                self.draw_centered(3, "do you want to exit?", None, w);
                self.draw_centered(4, "press y to exit", None, w);
                self.flush();

                if rx.recv_timeout(Duration::from_secs(5)).ok()?.code == KeyCode::Char('y') {
                    Some(Event::Exit)
                } else {
                    None
                }
            }
            KeyCode::Char('s') => Some(Event::Shuffle),
            KeyCode::Right => Some(Event::Next),
            KeyCode::Left => Some(Event::Prev),
            KeyCode::Char(' ') => Some(Event::PlayPause),
            _ => None,
        }
    }

    pub fn draw(&mut self, queue: &Queue, player: &Player) {
        let (w, _) = term::size_or();

        let current = if let Some(song) = queue.current() {
            &song.name
        } else {
            "<no song playing>"
        };
        self.draw_centered(2, current, Some("now: "), w);

        let next = if let Some(song) = queue.next() {
            &song.name
        } else {
            "<no song is next>"
        };
        self.draw_centered(3, next, Some("next: "), w);

        self.draw_centered(
            5,
            if player.playing() {
                "playing"
            } else {
                "paused"
            },
            None,
            w,
        );

        if let Some((elapsed, total)) = player.time_info() {
            self.draw_centered(6, &fmt_time(elapsed, total), None, w);
        }
    }

    pub fn clear(&mut self) {
        self.messages = 0;
        clear::all();
    }

    pub fn add_message(&mut self, message: Message) {
        let msg = &*message;

        let (w, h) = term::size_or();

        let mid = (w / 2).saturating_sub(msg.len() as u32 / 2);
        goto::pos(mid, h - self.messages);
        print!("{msg}");

        self.messages += 1;
    }

    pub fn flush(&self) {
        goto::home();
        cod::flush();
    }

    fn draw_centered(&mut self, y: u32, msg: &str, pre: Option<&str>, w: u32) {
        let mid = (w / 2)
            .saturating_sub((msg.len() as u32 + pre.map(|s| s.len() as u32).unwrap_or(0)) / 2);
        goto::pos(mid, y);

        if let Some(pre) = pre {
            print!("{pre}");
        }
        style::with::bold(|| print!("{msg}"));
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
    } else if emins > 0 || tmins > 0 {
        format!("{emins:02}:{esecs:02} / {tmins:02}:{tsecs:02}")
    } else {
        format!("{esecs:02} / {tsecs:02}")
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        term::primary_screen();
        term::disable_raw_mode();
    }
}
