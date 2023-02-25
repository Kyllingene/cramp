use std::env;
use std::fmt::Debug;
use std::fs::{read_dir, read_to_string, File};
use std::io::{self, stdin, stdout, BufReader};
use std::path::Path;
use std::time::Duration;

use crossbeam_channel::unbounded;
use dbus::blocking::LocalConnection;
use dbus::MethodErr;
use dbus_tree::{Access, Factory};
use rand::seq::SliceRandom;
use rand::thread_rng;
use rodio::{Decoder, OutputStream, Sink};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

#[derive(Debug, Clone)]
enum Message {
    SetVolume(f64),
    GetVolume,

    SetRate(f64),
    GetRate,

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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Song {
    pub file: String,
    pub name: String,
    save_name: Option<String>,

    pub next: Option<String>,

    pub length: Option<u32>,
}

impl Song {
    pub fn new<S: ToString>(
        file: S,
        name: Option<S>,
        next: Option<S>,
        length: Option<u32>,
    ) -> Self {
        Self {
            file: file.to_string(),
            save_name: name.as_ref().map(|s| s.to_string()),
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
        Decoder::new(BufReader::new(File::open(&self.file)?))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
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

    volume: f32,

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

            volume: 1.0,

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

                name = Some(bits.into_iter().skip(1).collect::<Vec<&str>>().join(","));
            } else if let Some(line) = line.strip_prefix("#::") {
                // TODO: make more compatible with M3U standards
                if let Some(s) = last.as_mut() {
                    s.next = Some(line[3..].to_string());
                }
            } else {
                if let Some(last) = last.take() {
                    songs.push(last);
                }

                last = Some(Song::new(line.to_string(), name.take(), None, length));
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

        writeln!(file, "#EXTM3U").unwrap();
        for song in &self.songs {
            if let Some(name) = &song.save_name {
                writeln!(file, "#EXTINF:{},{name}", song.length.unwrap_or(0)).unwrap();
            }

            if let Some(next) = &song.next {
                writeln!(file, "#::{next}").unwrap();
            }

            writeln!(file, "{}", song.file).unwrap();
        }
    }

    pub fn play(&mut self) {
        if self.sink.empty() {
            if let Some(song) = &self.current {
                self.sink.append(song.open().unwrap());
            }
        }

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

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        self.sink.set_volume(volume);
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        self.pause();
    }

    pub fn next(&mut self) {
        self.sink.stop();

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
        self.sink.stop();
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

    pub fn volume(&self) -> f64 {
        self.sink.volume() as f64
    }

    pub fn paused(&self) -> bool {
        self.sink.is_paused()
    }

    pub fn add_file<P: AsRef<Path>>(&mut self, file: P) -> io::Result<()> {
        let file = file.as_ref();

        if file.is_file() {
            self.songs.push(Song::new(file.display(), None, None, None));
        }

        Ok(())
    }
}

fn main() {
    let (tx, rx) = unbounded();

    let tx2 = tx.clone();
    std::thread::spawn(move || {
    for key in stdin().keys().flatten() {
            match key {
                Key::Char(' ') => tx2.send(Message::PlayPause).unwrap(),
                Key::Char('p') => tx2.send(Message::Pause).unwrap(),
                Key::Char('\n') => tx2.send(Message::Play).unwrap(),

                Key::Char('s') => tx2.send(Message::Shuffle).unwrap(),

                Key::Char('x') => tx2.send(Message::Stop).unwrap(),

                Key::Left => tx2.send(Message::Prev).unwrap(),
                Key::Right => tx2.send(Message::Next).unwrap(),

                Key::Char('q') | Key::Esc => {
                    tx2.send(Message::Exit).unwrap();
                    return;
                }

                _ => {}
            }
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

    let conn = LocalConnection::new_session().unwrap();
    conn.request_name("org.mpris.MediaPlayer2.cramp", false, true, false)
        .expect("Failed to register dbus name");

    let f = Factory::new_fn::<()>();

    let tx_next = tx.clone();
    let tx_prev = tx.clone();
    let tx_play = tx.clone();
    let tx_pause = tx.clone();
    let tx_playpause = tx.clone();
    let tx_stop = tx.clone();

    let tx_get_vol = tx.clone();
    let tx_set_vol = tx.clone();
    let (tx_vol, rx_vol) = unbounded();
    let tx_get_rate = tx.clone();
    let tx_set_rate = tx.clone();
    let (tx_rate, rx_rate) = unbounded();
    let tx_get_stat = tx.clone();
    let tx_set_shuf = tx.clone();
    let (tx_stat, rx_stat) = unbounded();
    let tx_open_uri = tx;

    let tree =
        f.tree(())
            .add(
                f.object_path("/org/mpris/MediaPlayer2", ())
                    .introspectable()
                    .add(
                        f.interface("org.mpris.MediaPlayer2", ())
                            .add_m(f.method("Quit", (), |_| std::process::exit(0)))
                            .add_m(f.method("Raise", (), |m| Ok(vec!(m.msg.method_return()))))
                            .add_p(
                                f.property::<bool, _>("CanQuit", ())
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanRaise", ())
                                    .on_get(|i, _| {
                                        i.append(false);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("HasTrackList", ())
                                    .on_get(|i, _| {
                                        i.append(false);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<&'static str, _>("Identity", ())
                                    .on_get(|i, _| {
                                        i.append("cramp");
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<Vec<&'static str>, _>("SupportedUriSchemes", ())
                                    .on_get(|i, _| {
                                        i.append(vec!["file"]);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<Vec<&'static str>, _>("SupportedMimeTypes", ())
                                    .on_get(|i, _| {
                                        i.append(vec![
                                            "audio/mpeg",
                                            "audio/ogg",
                                            "audio/wav",
                                            "audio/flac",
                                            "audio/vorbis",
                                        ]);
                                        Ok(())
                                    }),
                            ),
                    )
                    .introspectable()
                    .add(
                        f.interface("org.mpris.MediaPlayer2.Player", ())
                            .add_m(f.method("Next", (), move |m| {
                                tx_next.send(Message::Next).unwrap();
                                Ok(vec!(m.msg.method_return()))
                            }))
                            .add_m(f.method("Previous", (), move |m| {
                                tx_prev.send(Message::Prev).unwrap();
                                Ok(vec!(m.msg.method_return()))
                            }))
                            .add_m(f.method("Play", (), move |m| {
                                tx_play.send(Message::Play).unwrap();
                                Ok(vec!(m.msg.method_return()))
                            }))
                            .add_m(f.method("Pause", (), move |m| {
                                tx_pause.send(Message::Pause).unwrap();
                                Ok(vec!(m.msg.method_return()))
                            }))
                            .add_m(f.method("PlayPause", (), move |m| {
                                tx_playpause.send(Message::PlayPause).unwrap();
                                eprintln!("PLAY-PAUSED\nPLAY-PAUSED\nPLAY-PAUSED\nPLAY-PAUSED\nPLAY-PAUSED\nPLAY-PAUSED\nPLAY-PAUSED\nPLAY-PAUSED");
                                Ok(vec!(m.msg.method_return()))
                            }))
                            .add_m(f.method("Stop", (), move |m| {
                                tx_stop.send(Message::Stop).unwrap();
                                Ok(vec!(m.msg.method_return()))
                            }))
                            .add_m(
                                f.method("Seek", (), |m| Ok(vec!(m.msg.method_return())))
                                    .inarg::<i32, _>("Offset"),
                            )
                            .add_m(
                                f.method("SetPosition", (), |m| Ok(vec!(m.msg.method_return())))
                                    .inarg::<&str, _>("TrackId")
                                    .inarg::<i64, _>("Position"),
                            )
                            .add_m(
                                f.method("OpenUri", (), move |m| {
                                    tx_open_uri
                                        .send(Message::OpenUri(m.msg.read1::<&str>()?.to_string()))
                                        .map_err(|e| MethodErr::failed(&e))?;

                                    Ok(vec!(m.msg.method_return()))
                                }).inarg::<&str, _>("Uri"),
                            )
                            .add_p(f.property::<&str, _>("PlaybackStatus", ()).on_get(
                                move |i, _| {
                                    tx_get_stat
                                        .send(Message::GetStatus)
                                        .map_err(|e| MethodErr::failed(&e))?;

                                    let stat = rx_stat
                                        .recv_timeout(Duration::from_millis(200))
                                        .map_err(|e| MethodErr::failed(&e))?;
                                    i.append(stat);
                                    Ok(())
                                },
                            ))
                            .add_p(
                                f.property::<f64, _>("Rate", ())
                                    .access(Access::ReadWrite)
                                    .on_get(move |i, _| {
                                        tx_get_rate
                                            .send(Message::GetRate)
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        let rate = rx_rate
                                            .recv_timeout(Duration::from_millis(200))
                                            .map_err(|e| MethodErr::failed(&e))?;
                                        i.append(rate);
                                        Ok(())
                                    })
                                    .on_set(move |i, _| {
                                        tx_set_rate
                                            .send(Message::SetRate(i.read()?))
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<f64, _>("Volume", ())
                                    .access(Access::ReadWrite)
                                    .on_get(move |i, _| {
                                        tx_get_vol
                                            .send(Message::GetVolume)
                                            .map_err(|e| MethodErr::failed(&e.to_string()))?;

                                        let vol = rx_vol
                                            .recv_timeout(Duration::from_millis(200))
                                            .map_err(|e| MethodErr::failed(&e.to_string()))?;
                                        i.append(vol);
                                        Ok(())
                                    })
                                    .on_set(move |i, _| {
                                        tx_set_vol
                                            .send(Message::SetVolume(i.read()?))
                                            .map_err(|e| MethodErr::failed(&e.to_string()))?;

                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("Shuffle", ())
                                    .access(Access::ReadWrite)
                                    .on_get(move |i, _| {
                                        i.append(true);
                                        Ok(())
                                    })
                                    .on_set(move |_, _| {
                                        tx_set_shuf
                                            .send(Message::Shuffle)
                                            .map_err(|e| MethodErr::failed(&e.to_string()))?;

                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<i64, _>("Position", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(0i64);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<f64, _>("MinimumRate", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(1.0);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<f64, _>("MaximumRate", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(1.0);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanGoNext", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanGoPrevious", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanPlay", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanPause", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanSeek", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(false);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanControl", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            ),
                    )
                    .introspectable(),
            )
            .add(f.object_path("/", ()).introspectable());

    tree.start_receive(&conn);
    let mut drawer = cod::Drawer::from(stdout().into_raw_mode().unwrap());
    loop {
        conn.process(Duration::from_millis(200)).unwrap();
        if !queue.paused() && queue.empty() && queue.current.is_none() {
            queue.next();
        }

        while let Ok(message) = rx.try_recv() {
            match message {
                Message::GetVolume => tx_vol.send(queue.volume()).unwrap(),
                Message::SetVolume(vol) => queue.set_volume(vol as f32),

                Message::GetRate => tx_rate.send(queue.sink.speed() as f64).unwrap(),
                Message::SetRate(rate) => queue.sink.set_speed(rate as f32),

                Message::GetStatus => tx_stat
                    .send(if queue.empty() {
                        "Stopped"
                    } else if queue.paused() {
                        "Paused"
                    } else {
                        "Playing"
                    })
                    .unwrap(),

                Message::Play => queue.play(),
                Message::Pause => queue.pause(),
                Message::PlayPause => queue.play_pause(),
                Message::Next => queue.next(),
                Message::Prev => queue.last(),
                Message::Stop => queue.stop(),
                Message::Shuffle => queue.shuffle(),

                Message::OpenUri(uri) => queue.add_file(uri).unwrap(),

                Message::Exit => {
                    queue.stop();
                    return;
                }
            }
        }

        drawer.clear();

        drawer.pixel('<', 0, 1);
        if let Some(song) = &queue.current {
            drawer.text(song.name.chars(), 3, 1);
        }

        drawer.text(
            format!(
                "{}{}",
                if queue.sink.is_paused() { "||" } else { " >" },
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
