use std::env;
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crossbeam_channel::unbounded;

mod ui;
// mod input;
mod mpris;
mod process;
mod queue;
mod song;

use queue::Queue;

#[derive(Debug, Clone)]
pub enum Message {
    SetVolume(f64),
    GetVolume,

    SetRate(f64),
    GetRate,

    GetShuffle,
    GetStatus,

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

// #[derive(Debug, Clone)]
// pub struct Draw {
//     song: Option<String>,
//     next: Option<String>,

//     paused: bool,
//     empty: bool,
// }

fn main() {
    let mut queue = if let Some(path) = env::args().nth(1) {
        let path = Path::new(&path);

        if path.is_dir() {
            Queue::load_dir(path)
        } else {
            Queue::load(path)
        }
    } else {
        Queue::new()
    };

    queue.queue_all();

    let queue = Arc::new(Mutex::new(queue));

    let (tx, rx) = unbounded();
    
    process::process(Arc::clone(&queue), tx.clone(), rx);

    ui::ui(Arc::clone(&queue), tx);
}
