use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender};

use crate::{mpris, queue::Queue, Message};

pub fn process(queue: Arc<Mutex<Queue>>, tx: Sender<Message>, rx: Receiver<Message>) {
    std::thread::spawn(move || {
        let mpris = mpris::mpris(tx.clone());
        'mainloop: loop {
            // handle MPRIS requests
            mpris.conn.process(Duration::from_millis(500)).unwrap();

            // auto-next
            let mut queue = queue.lock().unwrap();

            if !queue.paused() && queue.empty() {
                queue.next();
            }

            // handle events (from keyboard and MPRIS)
            while let Ok(message) = rx.try_recv() {
                match message {
                    Message::GetVolume => mpris.vol.send(queue.volume() as f64).unwrap(),
                    Message::SetVolume(vol) => queue.set_volume(vol as f32),

                    Message::GetRate => mpris.rate.send(queue.speed() as f64).unwrap(),
                    Message::SetRate(rate) => queue.set_speed(rate as f32),

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
                        break 'mainloop;
                    }
                }
            }
        }
    });
}
