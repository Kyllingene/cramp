use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender};
use zbus::zvariant::Value;

use crate::{
    mpris,
    queue::{LoopMode, Queue},
    Message,
};

pub async fn process(queue: Arc<Mutex<Queue>>, tx: Sender<Message>, rx: Receiver<Message>) {
    tokio::spawn(async move {
        let (iface, mpris) = mpris::mpris(tx.clone()).await;

        let iface = Arc::new(iface);

        'mainloop: loop {
            {
                // auto-next
                let mut queue = queue.lock().unwrap();

                if !queue.paused() && queue.empty() {
                    queue.next();
                }
            }

            // handle events (from keyboard and MPRIS)
            while let Ok(message) = rx.try_recv() {
                match message {
                    Message::GetVolume => {
                        let queue = queue.lock().unwrap();
                        mpris.vol.send(queue.volume() as f64).unwrap();
                    }
                    Message::SetVolume(vol) => {
                        let mut queue = queue.lock().unwrap();
                        queue.set_volume(vol as f32);
                    }

                    Message::GetRate => {
                        let queue = queue.lock().unwrap();
                        mpris.rate.send(queue.speed() as f64).unwrap();
                    }
                    Message::SetRate(rate) => {
                        let queue = queue.lock().unwrap();
                        queue.set_speed(rate as f32);
                    }

                    Message::GetLoop => {
                        let queue = queue.lock().unwrap();
                        mpris.loop_mode.send(queue.loop_mode.to_string()).unwrap();
                    }
                    Message::SetLoop(loop_mode) => {
                        let mut queue = queue.lock().unwrap();
                        match loop_mode.as_str() {
                            "None" => queue.loop_mode = LoopMode::None,
                            "Track" => queue.loop_mode = LoopMode::Track,
                            "Playlist" => queue.loop_mode = LoopMode::Playlist,
                            _ => {}
                        }
                    }

                    Message::GetShuffle => {
                        let queue = queue.lock().unwrap();
                        mpris.shuf.send(queue.shuffle).unwrap();
                    }
                    Message::GetStatus => {
                        let queue = queue.lock().unwrap();
                        mpris
                            .stat
                            .send(if queue.empty() {
                                "Stopped"
                            } else if queue.paused() {
                                "Paused"
                            } else {
                                "Playing"
                            })
                            .unwrap();
                    }

                    Message::GetMetadata => {
                        let mut map = HashMap::new();

                        let current = { queue.lock().unwrap().current.clone() };

                        if let Some(current) = current {
                            map.insert(
                                "mpris:trackid".to_string(),
                                Value::U64(current.id).to_owned(),
                            );
                            map.insert(
                                "xesam:title".to_string(),
                                Value::Str(current.name.clone().into()).to_owned(),
                            );

                            if let Some(Ok(length)) = current.length.map(|l| l.try_into()) {
                                map.insert(
                                    "mpris:length".to_string(),
                                    Value::U64(length).to_owned(),
                                );
                            }
                        }

                        mpris.meta.send(map).unwrap();
                        let i = iface.get_mut().await;

                        i.metadata_changed(iface.signal_context()).await.unwrap();
                    }

                    Message::Play => {
                        let mut queue = queue.lock().unwrap();
                        queue.play();
                    }

                    Message::Pause => {
                        let mut queue = queue.lock().unwrap();
                        queue.pause();
                    }

                    Message::PlayPause => {
                        let mut queue = queue.lock().unwrap();
                        queue.play_pause();
                    }

                    Message::Next => {
                        let mut queue = queue.lock().unwrap();
                        queue.next();
                    }

                    Message::Prev => {
                        let mut queue = queue.lock().unwrap();
                        queue.last();
                    }

                    Message::Stop => {
                        let mut queue = queue.lock().unwrap();
                        queue.stop();
                    }

                    Message::Shuffle => {
                        let mut queue = queue.lock().unwrap();
                        queue.shuffle();
                    }

                    Message::OpenUri(uri) => {
                        let mut queue = queue.lock().unwrap();
                        queue.add_file(uri).unwrap();
                    }

                    Message::Exit => {
                        let mut queue = queue.lock().unwrap();
                        queue.stop();
                        break 'mainloop;
                    }
                }
            }
        }
    });
}
