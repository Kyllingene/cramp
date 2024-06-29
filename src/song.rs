use std::path::PathBuf;

use kittyaudio::{KaError, Sound};

pub struct Song {
    pub name: String,
    pub path: PathBuf,
    pub next: Option<PathBuf>,
    pub no_shuffle: bool,
}

impl Song {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path
            .file_name()
            .expect("attempted to load non-file as song")
            .to_string_lossy()
            .to_string();

        Self {
            name,
            path,
            next: None,
            no_shuffle: false,
        }
    }

    pub fn next(mut self, next: Option<impl Into<PathBuf>>) -> Self {
        self.next = next.map(Into::into);
        self
    }

    pub fn no_shuffle(mut self, no_shuffle: bool) -> Self {
        self.no_shuffle = no_shuffle;
        self
    }

    pub fn load(&self) -> Result<Sound, KaError> {
        Sound::from_path(&self.path)
    }
}
