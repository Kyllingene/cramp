use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use async_std::sync::Mutex;
use mpris_server::{Metadata, PlaybackStatus, Property, Signal, Time, TrackId};
use slotmap::Key;

use crate::player::Player;
use crate::queue::Queue;
use crate::song::Song;
use crate::ui::{Event, Ui};
use crate::Message;

pub enum Effect {
    Signal(Signal),
    Changed(Vec<Property>),
}

pub struct App {
    player: Mutex<Player>,
    queue: Mutex<Queue>,
    ui: Mutex<Ui>,

    pub quit: AtomicBool,
    pub effects: Mutex<Vec<Effect>>,
}

impl App {
    pub async fn new(path: impl AsRef<Path>) -> Self {
        let mut player = Player::new();
        let mut queue = Queue::new();
        let mut ui = Ui::new().await;

        for message in queue.load(path) {
            ui.add_message(message);
        }

        queue.queue_all();
        queue.shuffle();
        queue.advance();

        if let Some(song) = queue.current() {
            if let Err(e) = player.play(song) {
                ui.add_message(Message::new(format!(
                    "failed to play {}: {e}",
                    song.path.display()
                )));
            }
        }

        Self {
            player: Mutex::new(player),
            queue: Mutex::new(queue),
            ui: Mutex::new(ui),

            quit: AtomicBool::new(false),
            effects: Mutex::new(Vec::with_capacity(4)),
        }
    }

    fn play(&self, song: &Song, player: &mut Player, ui: &mut Ui) {
        if let Err(e) = player.play(song) {
            ui.add_message(Message::new(format!(
                "failed to play song {}: {e}",
                song.path.display()
            )));
        }
    }

    fn next(&self, song: &Song, player: &mut Player, ui: &mut Ui) {
        if let Err(e) = player.load_next(song) {
            ui.add_message(Message::new(format!(
                "failed to load song {}: {e}",
                song.path.display()
            )));
        }
    }

    fn advance(&self, player: &mut Player, queue: &mut Queue, ui: &mut Ui) {
        queue.advance();
        if let Some(song) = queue.current() {
            self.play(song, player, ui);
        }
        if let Some(song) = queue.next() {
            self.next(song, player, ui);
        }
    }

    fn previous(&self, player: &mut Player, queue: &mut Queue, ui: &mut Ui) {
        queue.previous();
        if let Some(song) = queue.current() {
            if let Err(e) = player.play(song) {
                ui.add_message(Message::new(format!(
                    "failed to play song {}: {e}",
                    song.path.display()
                )));
            }
            if let Some(song) = queue.next() {
                if let Err(e) = player.load_next(song) {
                    ui.add_message(Message::new(format!(
                        "failed to load song {}: {e}",
                        song.path.display()
                    )));
                }
            }
        }
    }

    pub async fn add_message(&self, message: Message) {
        self.ui.lock().await.add_message(message);
    }

    fn meta(&self, player: &Player, queue: &Queue) -> Metadata {
        let mut meta = Metadata::new();

        meta.set_trackid(Some(if let Some(id) = queue.current_id() {
            format!("/com/cramp/tracks/trackid{}", id.data().as_ffi())
                .try_into()
                .unwrap()
        } else {
            TrackId::NO_TRACK
        }));

        if let Some((_, secs)) = player.time_info() {
            let millis = (secs * 1000.0) as i64;
            let time = Time::from_millis(millis);
            meta.set_length(Some(time));
        }

        meta
    }

    fn status(&self, player: &Player) -> PlaybackStatus {
        match (player.playing(), player.finished()) {
            (false, false) => PlaybackStatus::Paused,
            (true, false) => PlaybackStatus::Playing,
            (false, true) | (true, true) => PlaybackStatus::Stopped,
        }
    }

    pub async fn poll(&self) {
        let mut player = self.player.lock().await;
        let mut queue = self.queue.lock().await;
        let mut ui = self.ui.lock().await;

        if self.quit.load(Ordering::Relaxed) {
            ui.exit();
            std::process::exit(0);
        }

        if player.finished() && player.playing() {
            self.advance(&mut player, &mut queue, &mut ui);
        }

        ui.draw(&queue, &player);
        ui.flush();

        // FIXME: query pause information from Queue, not Player
        if let Some(effect) = match ui.event(&queue).await {
            Some(Event::Exit) => {
                self.quit.store(true, Ordering::Relaxed);
                None
            }
            Some(Event::Shuffle) => {
                queue.shuffle();
                Some(Effect::Changed(vec![Property::Shuffle(true)]))
            }
            Some(Event::Next) => {
                player.end();
                self.advance(&mut player, &mut queue, &mut ui);

                Some(Effect::Changed(vec![
                    Property::Metadata(self.meta(&player, &queue)),
                    Property::PlaybackStatus(self.status(&player)),
                ]))
            }
            Some(Event::Prev) => {
                player.end();
                self.previous(&mut player, &mut queue, &mut ui);

                Some(Effect::Changed(vec![
                    Property::Metadata(self.meta(&player, &queue)),
                    Property::PlaybackStatus(self.status(&player)),
                ]))
            }
            Some(Event::PlayPause) => {
                if player.playing() {
                    player.pause();
                } else {
                    player.resume();
                }

                Some(Effect::Changed(vec![Property::PlaybackStatus(
                    self.status(&player),
                )]))
            }
            Some(Event::Play) => {
                if !player.playing() {
                    player.resume();
                }

                Some(Effect::Changed(vec![Property::PlaybackStatus(
                    self.status(&player),
                )]))
            }
            Some(Event::Pause) => {
                if player.playing() {
                    player.pause();
                }

                Some(Effect::Changed(vec![Property::PlaybackStatus(
                    self.status(&player),
                )]))
            }
            Some(Event::PlayNow(id)) => {
                let song = queue.play(id);
                if let Err(e) = player.play(song) {
                    ui.add_message(Message::new(format!(
                        "failed to play song {}: {e}",
                        song.path.display()
                    )));
                }

                Some(Effect::Changed(vec![
                    Property::Metadata(self.meta(&player, &queue)),
                    Property::PlaybackStatus(self.status(&player)),
                ]))
            }
            Some(Event::PlayNext(id)) => {
                let song = queue.set_next(id);
                if let Err(e) = player.load_next(song) {
                    ui.add_message(Message::new(format!(
                        "failed to load song {}: {e}",
                        song.path.display()
                    )));
                }

                None
            }
            Some(Event::Queue(id)) => {
                queue.queue(id, true);
                None
            }
            Some(Event::SeekRight) => {
                player.seek_by(5.0);
                player.time_info().map(|(secs, _)| {
                    let time = Time::from_millis((secs * 1000.0) as i64);
                    Effect::Signal(Signal::Seeked { position: time })
                })
            }
            Some(Event::SeekLeft) => {
                player.seek_by(-5.0);
                player.time_info().map(|(secs, _)| {
                    let time = Time::from_millis((secs * 1000.0) as i64);
                    Effect::Signal(Signal::Seeked { position: time })
                })
            }
            None => None,
        } {
            self.effects.lock().await.push(effect);
        }

        ui.clear();
    }
}

mod mpris {
    use std::sync::atomic::Ordering;

    use mpris_server::zbus::fdo::{Error as FError, Result as FResult};
    use mpris_server::zbus::Result as ZResult;
    use mpris_server::{
        LoopStatus, Metadata, PlaybackStatus, PlayerInterface, Property, RootInterface, Signal,
        Time, TrackId,
    };
    use slotmap::Key;

    use crate::song::Song;

    use super::{App, Effect};

    impl RootInterface for App {
        async fn identity(&self) -> FResult<String> {
            Ok("CRAMP".into())
        }

        async fn desktop_entry(&self) -> FResult<String> {
            Ok("cramp".into())
        }

        async fn supported_uri_schemes(&self) -> FResult<Vec<String>> {
            Ok(vec!["file".into()])
        }

        async fn supported_mime_types(&self) -> FResult<Vec<String>> {
            Ok(vec![
                "audio/mpeg".into(),
                "audio/ogg".into(),
                "audio/vorbis".into(),
                "audio/vnd.wav".into(),
            ])
        }

        async fn raise(&self) -> FResult<()> {
            Ok(())
        }

        async fn can_raise(&self) -> FResult<bool> {
            Ok(false)
        }

        async fn quit(&self) -> FResult<()> {
            self.quit.store(true, Ordering::Relaxed);
            Ok(())
        }

        async fn can_quit(&self) -> FResult<bool> {
            Ok(true)
        }

        async fn fullscreen(&self) -> FResult<bool> {
            Ok(false)
        }

        async fn set_fullscreen(&self, _: bool) -> ZResult<()> {
            Ok(())
        }

        async fn can_set_fullscreen(&self) -> FResult<bool> {
            Ok(false)
        }

        async fn has_track_list(&self) -> FResult<bool> {
            Ok(false)
        }
    }

    impl PlayerInterface for App {
        async fn next(&self) -> FResult<()> {
            let mut player = self.player.lock().await;
            let mut queue = self.queue.lock().await;
            let mut ui = self.ui.lock().await;
            self.advance(&mut player, &mut queue, &mut ui);

            let status = match (player.playing(), player.finished()) {
                (false, false) => PlaybackStatus::Paused,
                (true, false) => PlaybackStatus::Playing,
                (false, true) | (true, true) => PlaybackStatus::Stopped,
            };
            self.effects.lock().await.push(Effect::Changed(vec![
                Property::Metadata(self.meta(&player, &queue)),
                Property::PlaybackStatus(status),
            ]));

            Ok(())
        }

        async fn previous(&self) -> FResult<()> {
            let mut player = self.player.lock().await;
            let mut queue = self.queue.lock().await;
            let mut ui = self.ui.lock().await;
            self.previous(&mut player, &mut queue, &mut ui);

            let status = match (player.playing(), player.finished()) {
                (false, false) => PlaybackStatus::Paused,
                (true, false) => PlaybackStatus::Playing,
                (false, true) | (true, true) => PlaybackStatus::Stopped,
            };
            self.effects.lock().await.push(Effect::Changed(vec![
                Property::Metadata(self.meta(&player, &queue)),
                Property::PlaybackStatus(status),
            ]));

            Ok(())
        }

        async fn play(&self) -> FResult<()> {
            let mut player = self.player.lock().await;
            if !player.playing() {
                player.resume();
            }

            let status = match (player.playing(), player.finished()) {
                (false, false) => PlaybackStatus::Paused,
                (true, false) => PlaybackStatus::Playing,
                (false, true) | (true, true) => PlaybackStatus::Stopped,
            };
            self.effects
                .lock()
                .await
                .push(Effect::Changed(vec![Property::PlaybackStatus(status)]));

            Ok(())
        }

        async fn pause(&self) -> FResult<()> {
            let mut player = self.player.lock().await;
            if player.playing() {
                player.pause();
            }

            let status = match (player.playing(), player.finished()) {
                (false, false) => PlaybackStatus::Paused,
                (true, false) => PlaybackStatus::Playing,
                (false, true) | (true, true) => PlaybackStatus::Stopped,
            };
            self.effects
                .lock()
                .await
                .push(Effect::Changed(vec![Property::PlaybackStatus(status)]));

            Ok(())
        }

        async fn play_pause(&self) -> FResult<()> {
            let mut player = self.player.lock().await;
            if player.playing() {
                player.pause();
            } else {
                player.resume();
            }

            let status = match (player.playing(), player.finished()) {
                (false, false) => PlaybackStatus::Paused,
                (true, false) => PlaybackStatus::Playing,
                (false, true) | (true, true) => PlaybackStatus::Stopped,
            };
            self.effects
                .lock()
                .await
                .push(Effect::Changed(vec![Property::PlaybackStatus(status)]));

            Ok(())
        }

        async fn stop(&self) -> FResult<()> {
            let mut player = self.player.lock().await;
            player.stop();

            self.effects
                .lock()
                .await
                .push(Effect::Changed(vec![Property::PlaybackStatus(
                    PlaybackStatus::Stopped,
                )]));

            Ok(())
        }

        async fn seek(&self, time: Time) -> FResult<()> {
            let mut player = self.player.lock().await;
            let secs = time.as_millis() as f64 / 1000.0;
            player.seek_by(secs);

            self.effects
                .lock()
                .await
                .push(Effect::Signal(Signal::Seeked { position: time }));
            Ok(())
        }

        async fn position(&self) -> FResult<Time> {
            let player = self.player.lock().await;
            let Some((secs, _)) = player.time_info() else {
                return Ok(Time::from_secs(0));
            };

            let time = Time::from_millis((secs * 1000.0) as i64);
            Ok(time)
        }

        async fn set_position(&self, track_id: TrackId, time: Time) -> FResult<()> {
            let mut player = self.player.lock().await;
            let queue = self.queue.lock().await;

            let Some(current) = queue.current_id() else {
                return Ok(());
            };

            if track_id.as_str() != format!("/com/cramp/tracks/trackid{}", current.data().as_ffi())
            {
                return Ok(());
            }

            let secs = time.as_millis() as f64 / 1000.0;
            player.seek_to(secs);

            self.effects
                .lock()
                .await
                .push(Effect::Signal(Signal::Seeked { position: time }));
            Ok(())
        }

        async fn open_uri(&self, path: String) -> FResult<()> {
            let Some(path) = path
                .strip_prefix("file://")
                .and_then(|p| urlencoding::decode(p).ok())
            else {
                return Err(FError::InvalidArgs(path));
            };

            let mut player = self.player.lock().await;
            let mut queue = self.queue.lock().await;
            let mut ui = self.ui.lock().await;

            let id = queue.add_song(Song::new(path.as_ref()));
            queue.play(id);
            if let Some(song) = queue.current() {
                self.play(song, &mut player, &mut ui);
            }

            let status = match (player.playing(), player.finished()) {
                (false, false) => PlaybackStatus::Paused,
                (true, false) => PlaybackStatus::Playing,
                (false, true) | (true, true) => PlaybackStatus::Stopped,
            };
            self.effects.lock().await.push(Effect::Changed(vec![
                Property::Metadata(self.meta(&player, &queue)),
                Property::PlaybackStatus(status),
            ]));

            Ok(())
        }

        async fn playback_status(&self) -> FResult<PlaybackStatus> {
            let player = self.player.lock().await;
            Ok(match (player.playing(), player.finished()) {
                (false, false) => PlaybackStatus::Paused,
                (true, false) => PlaybackStatus::Playing,
                (false, true) | (true, true) => PlaybackStatus::Stopped,
            })
        }

        async fn loop_status(&self) -> FResult<LoopStatus> {
            Ok(LoopStatus::Playlist) // TODO: implement looping
        }

        async fn set_loop_status(&self, _: LoopStatus) -> ZResult<()> {
            Ok(())
        }

        async fn rate(&self) -> FResult<f64> {
            Ok(1.0) // TODO: implement rate
        }

        async fn minimum_rate(&self) -> FResult<f64> {
            Ok(1.0)
        }

        async fn maximum_rate(&self) -> FResult<f64> {
            Ok(1.0)
        }

        async fn set_rate(&self, _: f64) -> ZResult<()> {
            Ok(())
        }

        async fn shuffle(&self) -> FResult<bool> {
            Ok(true) // TODO: implement shuffling
        }

        async fn set_shuffle(&self, _: bool) -> ZResult<()> {
            let mut player = self.player.lock().await;
            let mut queue = self.queue.lock().await;
            let mut ui = self.ui.lock().await;

            queue.shuffle();
            if let Some(song) = queue.current() {
                self.play(song, &mut player, &mut ui);
            }

            self.effects
                .lock()
                .await
                .push(Effect::Changed(vec![Property::Shuffle(true)]));

            Ok(())
        }

        async fn metadata(&self) -> FResult<Metadata> {
            let player = self.player.lock().await;
            let queue = self.queue.lock().await;
            Ok(self.meta(&player, &queue))
        }

        async fn volume(&self) -> FResult<f64> {
            Ok(0.5) // TODO: implement volume
        }

        async fn set_volume(&self, _: f64) -> ZResult<()> {
            Ok(())
        }

        async fn can_go_next(&self) -> FResult<bool> {
            Ok(true)
        }

        async fn can_go_previous(&self) -> FResult<bool> {
            Ok(true)
        }

        async fn can_play(&self) -> FResult<bool> {
            Ok(true)
        }

        async fn can_pause(&self) -> FResult<bool> {
            Ok(true)
        }

        async fn can_seek(&self) -> FResult<bool> {
            Ok(true)
        }

        async fn can_control(&self) -> FResult<bool> {
            Ok(true)
        }
    }
}
