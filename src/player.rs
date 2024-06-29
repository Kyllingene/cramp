use kittyaudio::{Mixer, Sound, SoundHandle, KaError};

use crate::song::Song;

pub struct Player {
    pub mixer: Mixer,
    current: Option<SoundHandle>,
    next: Option<Sound>,
}

impl Player {
    pub fn new() -> Self {
        let mixer = Mixer::new();
        mixer.init();
        Self { mixer, current: None, next: None }
    }

    pub fn play(&mut self, song: &Song) -> Result<(), KaError> {
        if let Some(song) = self.current.take() {
            song.seek_to_end();
        }

        self.current = Some(self.mixer.play(song.load()?));
        Ok(())
    }
}
