use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, Stream, GenericFilePath, ListenerNonblockingMode, ListenerOptions};
use interprocess::os::unix::local_socket::ListenerOptionsExt;

use std::io::Read;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use crate::ui::Ui;
use crate::Message;

pub fn new() -> Listener {
    let ipc = ListenerOptions::new()
        .name(
            "/home/common/cramp/cramp.sock"
                .to_fs_name::<GenericFilePath>()
                .unwrap(),
        )
        .nonblocking(ListenerNonblockingMode::Accept)
        .mode(0o666) // give everyone permission to the socket
        .create_sync()
        .unwrap();

    // interprocess seems to be failing to change permissions, so just in case
    let perms = fs::Permissions::from_mode(0o666);
    fs::set_permissions("/home/common/cramp/cramp.sock", perms).unwrap();

    ipc
}

pub fn handle(conn: &Stream, ui: &mut Ui) -> Option<Event> {
    Event::parse_from_stream(conn)
        .inspect(|m| {
            ui.add_message(Message::stc(m.to_str()));
        })
        .inspect_err(|e| {
            ui.add_message(Message::new(format!("failed to handle ipc: {e}",)));
        })
        .ok()
}

pub enum Event {
    Exit,
    Shuffle,

    PlayPause,
    Play,
    Pause,

    Next,
    Prev,
}

impl Event {
    pub const MAX_STR_LEN: usize = Self::PlayPause.to_str().len();

    pub const fn to_str(&self) -> &'static str {
        match self {
            Self::Exit => "exit",
            Self::Shuffle => "shuffle",

            Self::PlayPause => "play-pause",
            Self::Play => "play",
            Self::Pause => "pause",

            Self::Next => "next",
            Self::Prev => "prev",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "exit" => Self::Exit,
            "shuffle" => Self::Shuffle,

            "playpause" | "play-pause" | "play_pause" => Self::PlayPause,
            "play" => Self::Play,
            "pause" => Self::Pause,

            "next" => Self::Next,
            "prev" | "previous" => Self::Prev,
            _ => return None,
        })
    }

    pub fn parse_from_stream(mut stream: &Stream) -> std::io::Result<Self> {
        let mut buf = [0u8; Self::MAX_STR_LEN];

        let mut slice = &mut buf[..];
        let mut len = 0;
        loop {
            let read = stream.read(slice)?;
            if read == 0 {
                break;
            }
            len += read;

            slice = &mut slice[read..];
        }

        let s = std::str::from_utf8(&buf[..len])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let s = s.trim();

        Self::from_str(s).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unrecognized command `{s}`"),
            )
        })
    }
}
