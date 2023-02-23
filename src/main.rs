use std::env;
use std::fmt::Debug;
use std::fs::{read_dir, File, read_to_string};
use std::io::{self, stdin, stdout, BufReader};
use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

use dbus::Message;
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rodio::{Decoder, OutputStream, Sink};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Song {
    pub file: String,
    pub name: String,

    pub next: Option<String>,

    pub length: Option<u32>,
}

impl Song {
    pub fn new<S: ToString>(file: S, name: Option<S>, next: Option<S>, length: Option<u32>) -> Self {
        Self {
            file: file.to_string(),
            name: name.map_or_else(
                || {
                    Path::new(&file.to_string())
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                },
                |s| s.to_string(),
            ),
            next: next.map(|s| s.to_string()),
            length,
        }
    }

    pub fn open(&self) -> io::Result<Decoder<BufReader<File>>> {
        Ok(Decoder::new(BufReader::new(File::open(&self.file)?))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?)
    }
}

struct LoadedSong {
    pub song: Song,
    pub data: Decoder<BufReader<File>>,
}

impl Debug for LoadedSong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.song)
    }
}

impl TryFrom<Song> for LoadedSong {
    type Error = io::Error;
    fn try_from(song: Song) -> Result<Self, Self::Error> {
        Ok(Self {
            data: song.open()?,
            song,
        })
    }
}

impl From<Song> for Option<LoadedSong> {
    fn from(song: Song) -> Self {
        Some(LoadedSong {
            data: song.open().ok()?,
            song,
        })
    }
}

impl From<&Song> for Option<LoadedSong> {
    fn from(song: &Song) -> Self {
        Some(LoadedSong {
            data: song.open().ok()?,
            song: song.clone(),
        })
    }
}

struct Queue {
    songs: Vec<Song>,

    current: Option<Song>,
    next: Option<LoadedSong>,

    queue: Vec<Song>,
    past: Vec<Song>,

    sink: Sink,
    _stream: OutputStream,
}

impl Default for Queue {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.pause();

        Self {
            songs: Vec::new(),

            current: None,
            next: None,

            queue: Vec::new(),
            past: Vec::with_capacity(100),

            sink,
            _stream,
        }
    }
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load<P: AsRef<Path>>(file: P) -> Self {
        let mut queue = Queue::new();

        let data = read_to_string(file).unwrap();

        let mut songs = Vec::new();
        let mut last: Option<Song> = None;

        let mut length = None;
        let mut name = None;

        for line in data.lines() {
            if line.starts_with("#EXTINF:") {
                let bits = line.split(',').collect::<Vec<&str>>();

                if let Ok(l) = bits[0].parse() {
                    length = Some(l);
                }

                if let Some(n) = bits.get(1) {
                    name = Some(*n);
                }
            } else if line.starts_with("::") {
                last.as_mut().map(|s| s.next = Some(line[2..].to_string()));
            } else {
                if let Some(last) = last.take() {
                    songs.push(last);
                }

                last = Some(Song::new(line, name, None, length));
            }
        }

        if let Some(last) = last.take() {
            songs.push(last);
        }

        queue.songs = songs;
        queue
    }

    pub fn load_dir<P: AsRef<Path>>(path: P) -> Self {
        Self {
            songs: Self::load_dir_entry(path),
            ..Default::default()
        }
    }

    fn load_dir_entry<P: AsRef<Path>>(dir: P) -> Vec<Song> {
        let mut songs = Vec::new();
        for entry in read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_dir() {
                songs.append(&mut Self::load_dir_entry(path));
            } else {
                songs.push(Song::new(path.display(), None, None, None));
            }
        }

        songs
    }

    pub fn save_playlist<P: AsRef<Path>>(&self, path: P) {
        use std::io::Write;
        let mut file = File::create(path).unwrap();

        for song in &self.songs {
            write!(file, "{}", song.file).unwrap();
            
            if let Some(next) = &song.next {
                write!(file, "\n::{next}\n").unwrap();
            } else {
                writeln!(file).unwrap();
            }
        } 
    }

    pub fn play(&mut self) {
        self.sink.play();
    }

    pub fn pause(&mut self) {
        self.sink.pause();
    }

    pub fn play_pause(&mut self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    pub fn raise_volume(&mut self) {
        self.sink.set_volume(self.sink.volume() + 0.2);
    }

    pub fn lower_volume(&mut self) {
        self.sink.set_volume(self.sink.volume() - 0.2);
    }

    pub fn stop(&mut self) {
        self.sink.stop();
    }

    pub fn next(&mut self) {
        self.stop();

        if let Some(song) = self.current.take() {
            self.past.push(song);
            self.past.reverse();
            self.past.truncate(100);
            self.past.reverse();
        }

        if self.queue.is_empty() {
            self.queue = self.songs.clone();
        }

        if let Some(song) = self.next.take() {
            self.current = Some(song.song);
            self.sink.append(song.data);
        }

        self.next = if let Some(Some(next)) = &self.current.as_ref().map(|s| s.next.clone()) {
            self.songs.iter().find(|s| &s.file == next).unwrap().into()
        } else if let Some(song) = self.queue.pop() {
            song.into()
        } else {
            None
        };
    }

    pub fn last(&mut self) {
        self.stop();
        if let Some(song) = self.current.take() {
            if let Some(song) = self.next.take() {
                self.queue.push(song.song);
            }

            self.next = song.into();
        }

        self.current = self.past.pop();
        if let Some(song) = &self.current {
            self.sink.append(song.open().unwrap());
        }
    }

    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn shuffle(&mut self) {
        self.queue.shuffle(&mut thread_rng());
        self.songs.shuffle(&mut thread_rng());
    }

    pub fn queue_all(&mut self) {
        self.queue = self.songs.clone();
    }

    pub fn empty(&self) -> bool {
        self.sink.empty()
    }

    pub fn volume(&self) -> f32 {
        self.sink.volume()
    }

    pub fn paused(&self) -> bool {
        self.sink.is_paused()
    }
}

fn handle_message(msg: &Message, tx: &Sender<Key>) {
    if let Some(member) = msg.member() {
        match member.to_string().as_str() {
            "Play" |
            "Pause" |
            "PlayPause" => tx.send(Key::Char(' ')).unwrap(),
            "Next" => tx.send(Key::Right).unwrap(),
            "Previous" => tx.send(Key::Left).unwrap(),
            "Shuffle" => tx.send(Key::Char('s')).unwrap(),
            _ => {}
        }
    }
}

fn main() {

    let (tx, rx) = channel();
    
    let tx2 = tx.clone();
    std::thread::spawn(move || {
        for key in stdin().keys() {
            tx2.send(key.unwrap()).unwrap();
        }
    });

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

    let mut rule = MatchRule::new().with_interface("org.mpris.MediaPlayer2.Player");

    let conn = Connection::new_session().unwrap();
    let proxy = conn.with_proxy("org.mpris.MediaPlayer2.cramp", "/org/mpris/MediaPlayer2", Duration::new(5, 0));

    let result: Result<(), dbus::Error> = proxy.method_call("org.freedesktop.DBus.Monitoring", "BecomeMonitor", (vec!(rule.match_str()), 0u32));

    let tx2 = tx.clone();
    if result.is_ok() {
        // Start matching using new scheme
        conn.start_receive(rule, Box::new(move |msg, _| {
            handle_message(&msg, &tx2);
            true
        }));
    } else {
        // Start matching using old scheme
        rule.eavesdrop = true; // this lets us eavesdrop on *all* session messages, not just ours
        conn.add_match(rule, move |_: (), _, msg| {
            handle_message(&msg, &tx2);
            true
        }).expect("add_match failed");
    }
    
    let mut drawer = cod::Drawer::from(stdout().into_raw_mode().unwrap());
    loop {
        if !queue.paused() && queue.empty() {
            queue.next();
        }

        conn.process(Duration::from_millis(1000)).unwrap();
        while let Ok(key) = rx.try_recv() {
            match key {
                Key::Char(' ') => queue.play_pause(),
                Key::Char('p') => queue.pause(),
                Key::Char('\n') => queue.play(),

                Key::Char('s') => queue.shuffle(),

                Key::Left => queue.last(),
                Key::Right => queue.next(),

                Key::Up => queue.raise_volume(),
                Key::Down => queue.lower_volume(),

                Key::Char('q') | Key::Esc => {
                    queue.stop();
                    return;
                }

                _ => {}
            }
        }

        drawer.clear();

        drawer.text("> ".chars(), 0, 1);
        if let Some(song) = &queue.current {
            drawer.text(song.name.chars(), 3, 1);
        }

        drawer.text(
            format!(
                "{}{}",
                if queue.sink.is_paused() {
                    "||"
                } else {
                    " >"
                },
                if queue.sink.empty() {
                    "\n  (no song)"
                } else {
                    ""
                }
            )
            .chars(),
            1,
            4,
        );

        if let Some(song) = &queue.next {
            drawer.text(format!("Next: {}", song.song.name).chars(), 2, 6);
        }

        drawer.bot();
        drawer.flush();
    
        std::thread::sleep(Duration::from_millis(200));
    }
}
