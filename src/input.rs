use std::io::stdin;

use crossbeam_channel::Sender;
use termion::event::Key;
use termion::input::TermRead;

use crate::Message;

pub fn get_input(tx: Sender<Message>) {
    std::thread::spawn(move || {
        for key in stdin().keys().flatten() {
            match key {
                Key::Char(' ') => tx.send(Message::PlayPause).unwrap(),
                Key::Char('p') => tx.send(Message::Pause).unwrap(),
                Key::Char('\n') => tx.send(Message::Play).unwrap(),

                Key::Char('s') => tx.send(Message::Shuffle).unwrap(),

                Key::Char('x') => tx.send(Message::Stop).unwrap(),

                Key::Left => tx.send(Message::Prev).unwrap(),
                Key::Right => tx.send(Message::Next).unwrap(),

                Key::Char('q') | Key::Esc => {
                    tx.send(Message::Exit).unwrap();
                    return;
                }

                _ => {}
            }
        }
    });
}