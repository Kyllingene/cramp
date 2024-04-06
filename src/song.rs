use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{self, BufReader};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rodio::{Decoder, Source};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Song {
    /// The filepath of the song
    pub file: String,
    /// The human-friendly name of the song
    pub name: String,
    /// The name to save the song under in a playlist (for songs with no given name)
    pub save_name: Option<String>,

    /// An optional "next-song-override", by filepath
    pub next: Option<String>,

    /// The length of the song in microseconds
    pub length: Option<u128>,

    /// Prevent the song from being played normally if the queue is shuffled
    pub noshuffle: bool,

    /// The unique ID of the song
    pub id: u64,
}

impl Song {
    /// Creates a new song
    pub fn new<S: ToString>(
        file: S,
        name: Option<S>,
        next: Option<S>,
        length: Option<u128>,
    ) -> Self {
        let mut file = file.to_string();
        if file.starts_with('~') {
            file = file.replace('~', dirs::home_dir().unwrap().to_str().unwrap());
        }

        let mut hasher = DefaultHasher::new();
        file.to_string().hash(&mut hasher);
        let id = hasher.finish();

        Self {
            file: file.to_string(),
            save_name: name.as_ref().map(|s| s.to_string()),
            // If given a name, use that
            // Otherwise, use the filename
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
            noshuffle: false,
            id,
        }
    }

    /// Set song.noshuffle
    pub fn noshuffle(mut self, noshuffle: bool) -> Self {
        self.noshuffle = noshuffle;
        self
    }

    /// Open a song (returns a rodio-ready decoder)
    /// Also attempts to get the length of the song
    pub fn open(&mut self) -> io::Result<Decoder<BufReader<File>>> {
        let dec = Decoder::new(BufReader::new(File::open(&self.file)?))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        // If the total duration is unavailable, but a length
        // was given by the playlist, don't override
        if let Some(len) = dec.total_duration().map(|d| d.as_micros()) {
            self.length = Some(len);
            // self.name, len / 60000000,
            // (len / 1000000) % 60,);
        }

        Ok(dec)
    }
}

// This is required in order to update the unique ID
impl Clone for Song {
    fn clone(&self) -> Self {
        Self {
            file: self.file.clone(),
            name: self.name.clone(),
            save_name: self.save_name.clone(),
            next: self.next.clone(),
            length: self.length,
            noshuffle: self.noshuffle,
            id: self.id.wrapping_add(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
        }
    }
}

// A song whose data has already been loaded
pub struct LoadedSong {
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
    fn try_from(mut song: Song) -> Result<Self, Self::Error> {
        Ok(Self {
            data: song.open()?,
            song,
        })
    }
}

impl From<Song> for Option<LoadedSong> {
    fn from(mut song: Song) -> Self {
        Some(LoadedSong {
            data: song.open().ok()?,
            song,
        })
    }
}

impl From<&mut Song> for Option<LoadedSong> {
    fn from(song: &mut Song) -> Self {
        Some(LoadedSong {
            data: song.open().ok()?,
            song: song.clone(),
        })
    }
}

impl LoadedSong {
    pub fn from(song: Option<Song>) -> Option<Self> {
        song?.into()
    }
}
