use std::env;
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crossbeam_channel::unbounded;

#[cfg(unix)]
mod mpris;

#[cfg(windows)]
mod controls;

mod process;
mod queue;
mod song;
mod ui;

use queue::Queue;

const PERSIST_FILENAME: &str = ".cramp-playlist.m3u";

#[derive(Debug, Clone)]
pub enum Message {
    SetVolume(f64),
    GetVolume,

    SetRate(f64),
    GetRate,

    SetLoop(String),
    GetLoop,

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

    if let Some(mut path) = dirs::home_dir() {
        path.push(PERSIST_FILENAME);

        if path.is_file() {
            playlist = Some(path);
        } else {
            eprintln!("Persisted playlist ({}) doesn't exist", path.display());
        }
    } else {
        eprintln!("Failed to check for persisted playlist");
    }

    if let Some(path) = env::args().nth(1) {
        playlist = Some(path.into());
    }

    let mut queue = if let Some(path) = &playlist {
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

    ui::ui(Arc::clone(&queue), tx, playlist);
}
