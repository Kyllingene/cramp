use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use rodio::{Decoder, Source};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Song {
    // the filepath of the song
    pub file: String,
    // the human-friendly name of the song
    pub name: String,
    // the name to save the song under in a playlist (for songs with no given name)
    pub save_name: Option<String>,

    // an optional "next-song-override", by filepath
    pub next: Option<String>,

    // the length of the song in microseconds
    pub length: Option<u128>,

    // prevents the song from being played normally if the queue is shuffled
    pub noshuffle: bool,
}

impl Song {
    // creates a new song
    pub fn new<S: ToString>(
        file: S,
        name: Option<S>,
        next: Option<S>,
        length: Option<u128>,
    ) -> Self {
        Self {
            file: file.to_string(),
            save_name: name.as_ref().map(|s| s.to_string()),
            // if given a name, use that
            // otherwise, use the filename
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
        }
    }

    // set song.noshuffle
    pub fn noshuffle(mut self, noshuffle: bool) -> Self {
        self.noshuffle = noshuffle;
        self
    }

    // open a song (returns a rodio-ready decoder)
    // also attempts to get the length of the song
    pub fn open(&mut self) -> io::Result<Decoder<BufReader<File>>> {
        let dec = Decoder::new(BufReader::new(File::open(&self.file)?))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        // if the total duration is unavailable, but a length
        // was given by the playlist, don't override
        if let Some(len) = dec.total_duration().map(|d| d.as_micros()) {
            self.length = Some(len);
        }

        Ok(dec)
    }
}

// a song whose data has already been loaded
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
