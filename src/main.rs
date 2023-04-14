use std::env;
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};

use std::sync::mpsc::channel;

mod mpris;
mod process;
mod queue;
mod song;
mod ui;

use queue::Queue;

#[derive(Debug, Clone)]
pub enum Message {
    SetVolume(f64),
    GetVolume,

    SetRate(f64),
    GetRate,

    GetShuffle,
    GetStatus,
    GetMetadata,

    Play,
    Pause,
    PlayPause,
    Next,
    Prev,
    Stop,
    Shuffle,
    Exit,

    OpenUri(String),
}

fn main() {
    let mut playlist = None;
    let mut queue = if let Some(path) = env::args().nth(1) {
        let path = Path::new(&path);

        if path.is_dir() {
            Queue::load_dir(path)
        } else {
            playlist = Some(path.to_path_buf());
            Queue::load(path)
        }
    } else {
        Queue::new()
    };

    queue.queue_all();

    let queue = Arc::new(Mutex::new(queue));

    let (tx, rx) = channel();

    process::process(Arc::clone(&queue), tx.clone(), rx);

    ui::ui(Arc::clone(&queue), tx, playlist);
}
