use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use crossbeam_channel::{Receiver, Sender};
use rand::random;

#[cfg(unix)]
use {
    crate::{mpris, queue::LoopMode},
    dbus::arg::{PropMap, Variant},
};

#[cfg(windows)]
use crate::controls;

use crate::queue::Queue;
use crate::Message;

pub fn process(queue: Arc<Mutex<Queue>>, tx: Sender<Message>, rx: Receiver<Message>) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        {
            let controls = controls::controls(tx.clone());
            'mainloop: loop {
                let mut queue = queue.lock().unwrap();

                // auto-next
                if !queue.paused() && queue.empty() {
                    queue.next();
                }

                while let Ok(message) = rx.try_recv() {
                    match message {
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
                            drop(controls);
                            break 'mainloop;
                        }

                        msg => panic!("Unhandled message: {msg:?}"),
                    }
                }
            }
        }

        #[cfg(unix)]
        {
            let mpris = mpris::mpris(tx.clone());
            let mut lastloop = Instant::now();
            let mut silence = 0.0;
            let mut wait = false;
            'mainloop: loop {
                let delta = (Instant::now() - lastloop).as_secs_f64();
                lastloop = Instant::now();
                let mut queue = queue.lock().unwrap();
                if !wait && silence == 0.0 && queue.do_silence() && queue.empty() && !queue.paused()
                {
                    silence = random::<f64>() * (queue.silence.end() - queue.silence.start())
                        + queue.silence.start();
                    wait = true;
                } else if wait && silence == 0.0 {
                    wait = false;
                }

                // auto-next
                if !wait && !queue.paused() && queue.empty() && silence == 0.0 {
                    queue.next();
                    wait = false;
                }

                if silence != 0.0 {
                    silence -= delta;
                    if silence <= 0.0 {
                        silence = 0.0;
                    }
                }

                // handle events from MPRIS
                while let Ok(message) = rx.try_recv() {
                    match message {
                        Message::GetVolume => mpris.vol.send(queue.volume() as f64).unwrap(),
                        Message::SetVolume(vol) => queue.set_volume(vol as f32),

                        Message::GetRate => mpris.rate.send(queue.speed() as f64).unwrap(),
                        Message::SetRate(rate) => queue.set_speed(rate as f32),

                        Message::GetLoop => {
                            mpris.loop_mode.send(queue.loop_mode.to_string()).unwrap()
                        }
                        Message::SetLoop(loop_mode) => match loop_mode.as_str() {
                            "None" => queue.loop_mode = LoopMode::None,
                            "Track" => queue.loop_mode = LoopMode::Track,
                            "Playlist" => queue.loop_mode = LoopMode::Playlist,
                            _ => {}
                        },

                        Message::GetShuffle => mpris.shuf.send(queue.shuffle).unwrap(),
                        Message::GetStatus => mpris
                            .stat
                            .send(if queue.empty() {
                                "Stopped"
                            } else if queue.paused() {
                                "Paused"
                            } else {
                                "Playing"
                            })
                            .unwrap(),

                        Message::GetMetadata => {
                            let mut map = PropMap::new();

                            if let Some(current) = &queue.current {
                                map.insert(
                                    "mpris:trackid".to_string(),
                                    Variant(Box::new(current.id)),
                                );
                                map.insert(
                                    "xesam:title".to_string(),
                                    Variant(Box::new(current.name.clone())),
                                );

                                if let Some(Ok(length)) = current.length.map(|l| l.try_into()) {
                                    let length: u64 = length;
                                    map.insert(
                                        "mpris:length".to_string(),
                                        Variant(Box::new(length)),
                                    );
                                }
                            }

                            mpris.meta.send(map).unwrap();
                        }

                        Message::Play => {
                            queue.play();
                            silence = 0.0;
                            wait = true;
                        }
                        Message::Pause => {
                            queue.pause();
                            silence = 0.0;
                            wait = true;
                        }
                        Message::PlayPause => {
                            queue.play_pause();
                            silence = 0.0;
                            wait = true;
                        }
                        Message::Next => {
                            queue.next();
                            silence = 0.0;
                            wait = true;
                        }
                        Message::Prev => {
                            queue.last();
                            silence = 0.0;
                            wait = true;
                        }
                        Message::Stop => {
                            queue.stop();
                            silence = 0.0;
                            wait = true;
                        }
                        Message::Shuffle => queue.shuffle(),

                        Message::OpenUri(uri) => queue.add_file(uri).unwrap(),

                        Message::Exit => {
                            queue.stop();
                            break 'mainloop;
                        }

                        #[allow(unreachable_patterns)]
                        msg => panic!("Unhandled message: {msg:?}"),
                    }
                }
            }
        }
    });
}
