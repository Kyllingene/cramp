use kittyaudio::{KaError, Mixer, Sound, SoundHandle};

use crate::song::Song;

pub struct Player {
    mixer: Mixer,
    current: Option<SoundHandle>,
    next: Option<Sound>,
}

impl Player {
    pub fn new() -> Self {
        let mixer = Mixer::new();
        mixer.init();
        Self {
            mixer,
            current: None,
            next: None,
        }
    }

    pub fn playing(&self) -> bool {
        self.current.as_ref().is_some_and(|c| !c.paused())
    }

    pub fn finished(&self) -> bool {
        self.current.as_ref().map(|c| c.finished()).unwrap_or(true)
    }

    /// Returns the time elapsed and total time, in that order, of the current song (if any).
    pub fn time_info(&self) -> Option<(f64, f64)> {
        self.current.as_ref().map(|c| {
            (
                c.index() as f64 / c.sample_rate() as f64,
                c.duration_seconds(),
            )
        })
    }

    pub fn resume(&mut self) {
        if let Some(current) = &self.current {
            current.resume();
        }
    }

    pub fn pause(&mut self) {
        if let Some(current) = &self.current {
            current.pause();
        }
    }

    pub fn stop(&mut self) {
        if let Some(song) = self.current.take() {
            song.seek_to_end();
            song.resume(); // TODO: is this necessary?
        }
    }

    pub fn play(&mut self, song: &Song) -> Result<(), KaError> {
        self.stop();
        self.current = Some(self.mixer.play(song.load()?));
        Ok(())
    }

    pub fn load_next(&mut self, song: &Song) -> Result<(), KaError> {
        self.next = Some(song.load()?);
        Ok(())
    }
}
