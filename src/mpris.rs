use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{dbus_interface, ConnectionBuilder, InterfaceRef};

use crate::queue::Queue;
use crate::Message;

// pub struct MprisRecv {
//     pub vol: Sender<f64>,
//     pub rate: Sender<f64>,
//     pub shuf: Sender<bool>,
//     pub stat: Sender<&'static str>,
//     pub meta: Sender<HashMap<String, OwnedValue>>,
//     pub loop_mode: Sender<String>,
// }

pub struct MprisHandler {
    queue: Arc<Mutex<Queue>>,
}

pub struct Player {
    queue: Arc<Mutex<Queue>>,
    iface: Arc<Mutex<MaybeUninit<InterfaceRef<Player>>>>,
}

impl MprisHandler {
    pub async fn new(queue: Arc<Mutex<Queue>>) {
        // let (tx_vol, rx_vol) = unbounded();
        // let (tx_rate, rx_rate) = unbounded();
        // let (tx_shuf, rx_shuf) = unbounded();
        // let (tx_stat, rx_stat) = unbounded();
        // let (tx_meta, rx_meta) = unbounded();
        // let (tx_loop_mode, rx_loop_mode) = unbounded();

        let me = Self {
            queue: queue.clone(),
        };

        let mut iface = Arc::new(Mutex::new(MaybeUninit::uninit()));
        let player = Player {
            queue,
            iface: iface.clone(),
        };

        let connection = ConnectionBuilder::session()
            .expect("Failed to connect to D-Bus")
            .name(format!(
                "org.mpris.MediaPlayer2.cramp.instance{}",
                std::process::id()
            ))
            .expect("Failed to get unique name")
            .serve_at("/org/mpris/MediaPlayer2", me)
            .expect("Failed to register base interface")
            .serve_at("/org/mpris/MediaPlayer2", player)
            .expect("Failed to register player interface")
            .build()
            .await
            .expect("Failed to build the connection");

        let iface_ref = connection
            .object_server()
            .interface::<_, Player>("/org/mpris/MediaPlayer2")
            .await
            .unwrap();

        iface.lock().expect("Failed to lock iface").write(iface_ref);

        tokio::spawn(async move {
            loop {
                connection.executor().tick().await;
            }
        });

        // let recv = MprisRecv {
        //     vol: tx_vol,
        //     rate: tx_rate,
        //     shuf: tx_shuf,
        //     stat: tx_stat,
        //     meta: tx_meta,
        //     loop_mode: tx_loop_mode,
        // };
    }
}

#[dbus_interface(name = "org.mpris.MediaPlayer2")]
impl MprisHandler {
    async fn quit(&self) {
        // self.tx_exit.send(Message::Exit).unwrap();
        self.queue.lock().expect("Failed to lock queue").quit = true;
    }

    async fn raise(&self) {}

    #[dbus_interface(property)]
    async fn can_quit(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    async fn can_raise(&self) -> bool {
        false
    }

    #[dbus_interface(property)]
    async fn fullscreen(&self) -> bool {
        false
    }

    #[dbus_interface(property)]
    async fn set_fullscreen(&mut self, _full: bool) {}

    #[dbus_interface(property)]
    async fn can_set_fullscreen(&self) -> bool {
        false
    }

    #[dbus_interface(property)]
    async fn identity(&self) -> &str {
        "CRAMP"
    }

    #[dbus_interface(property)]
    async fn has_track_list(&self) -> bool {
        false
    }

    #[dbus_interface(property)]
    async fn supported_uri_schemes(&self) -> Vec<&str> {
        vec!["file"]
    }

    #[dbus_interface(property)]
    async fn supported_mine_types(&self) -> Vec<&str> {
        vec![
            "audio/mpeg",
            "audio/ogg",
            "audio/wav",
            "audio/flac",
            "audio/vorbis",
        ]
    }
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn next(&self) {
        // self.tx_next.send(Message::Next).unwrap();
        self.queue.lock().expect("Failed to lock queue").next();
    }

    async fn previous(&self) {
        // self.tx_prev.send(Message::Prev).unwrap();
        self.queue.lock().expect("Failed to lock queue").last();
    }

    async fn play(&self) {
        // self.tx_play.send(Message::Play).unwrap();
        self.queue.lock().expect("Failed to lock queue").play();
    }

    async fn pause(&self) {
        // self.tx_pause.send(Message::Pause).unwrap();
        self.queue.lock().expect("Failed to lock queue").pause();
    }

    async fn play_pause(&self) {
        // self.tx_playpause.send(Message::PlayPause).unwrap();
        self.queue
            .lock()
            .expect("Failed to lock queue")
            .play_pause();
    }

    async fn stop(&self) {
        // self.tx_stop.send(Message::Stop).unwrap();
        self.queue.lock().expect("Failed to lock queue").stop();
    }

    async fn seek(&self, _offset: i64) {}

    async fn set_position(&self, _track_id: String, _position: i64) {}

    async fn open_uri(&self, uri: String) {
        // self.tx_open_uri.send(Message::OpenUri(uri)).unwrap();
        self.queue
            .lock()
            .expect("Failed to lock queue")
            .add_file(uri)
            .expect("Failed to open URI");
    }

    #[dbus_interface(property)]
    async fn playback_status(&self) -> &str {
        // self.tx_get_stat.send(Message::GetStatus).unwrap();

        // let stat = self
        //     .rx_stat
        //     .recv_timeout(Duration::from_millis(200))
        //     .unwrap();

        // stat

        let queue = self.queue.lock().expect("Failed to lock queue");
        if queue.empty() {
            "Stopped"
        } else if queue.paused() {
            "Paused"
        } else {
            "Playing"
        }
    }

    // b.property("LoopStatus")
    #[dbus_interface(property)]
    async fn loop_status(&self) -> String {
        // self.tx_get_loop_mode.send(Message::GetLoop).unwrap();

        // let loop_mode = self
        //     .rx_loop_mode
        //     .recv_timeout(Duration::from_millis(200))
        //     .unwrap();

        // loop_mode

        self.queue
            .lock()
            .expect("Failed to lock queue")
            .loop_mode
            .to_string()
    }
    #[dbus_interface(property)]
    async fn set_loop_status(&mut self, loop_mode: String) {
        // self.tx_set_loop_mode
        //     .send(Message::SetLoop(loop_mode))
        //     .unwrap();

        self.queue.lock().expect("Failed to lock queue").loop_mode = loop_mode.into();
    }

    #[dbus_interface(property)]
    async fn rate(&self) -> u64 {
        // self.tx_get_rate.send(Message::GetRate).unwrap();

        // let rate = self
        //     .rx_rate
        //     .recv_timeout(Duration::from_millis(200))
        //     .unwrap();

        // rate as u64

        self.queue.lock().expect("Failed to lock queue").speed() as u64
    }

    #[dbus_interface(property)]
    async fn set_rate(&mut self, rate: u64) {
        // self.tx_set_rate
        //     .send(Message::SetRate(rate as f64))
        //     .unwrap();

        self.queue
            .lock()
            .expect("Failed to lock queue")
            .set_speed(rate as f32)
    }

    #[dbus_interface(property)]
    async fn volume(&self) -> f64 {
        // self.tx_get_vol.send(Message::GetVolume).unwrap();

        // let vol = self
        //     .rx_vol
        //     .recv_timeout(Duration::from_millis(200))
        //     .unwrap();

        // vol

        self.queue.lock().expect("Failed to lock queue").volume() as f64
    }

    #[dbus_interface(property)]
    async fn set_volume(&mut self, volume: f64) {
        // self.tx_set_vol.send(Message::SetVolume(volume)).unwrap();
        self.queue
            .lock()
            .expect("Failed to lock queue")
            .set_volume(volume as f32)
    }

    #[dbus_interface(property)]
    async fn shuffle(&self) -> bool {
        // self.tx_get_shuf.send(Message::GetShuffle).unwrap();

        // let shuf = self
        //     .rx_shuf
        //     .recv_timeout(Duration::from_millis(200))
        //     .unwrap();

        // shuf

        self.queue.lock().expect("Failed to lock queue").shuffle
    }

    #[dbus_interface(property)]
    async fn set_shuffle(&mut self, _shuf: bool) {
        // self.tx_set_shuf.send(Message::Shuffle).unwrap();
        self.queue.lock().expect("Failed to lock queue").shuffle()
    }

    #[dbus_interface(property)]
    async fn metadata(&self) -> HashMap<String, OwnedValue> {
        // self.tx_get_meta.send(Message::GetMetadata).unwrap();

        // let meta = self
        //     .rx_meta
        //     .recv_timeout(Duration::from_millis(200))
        //     .unwrap();

        // meta

        let mut map = HashMap::new();

        let current = self.queue.lock().unwrap().current.clone();

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
                map.insert("mpris:length".to_string(), Value::U64(length).to_owned());
            }
        }

        // let i = iface.get_mut().await;
        // i.metadata_changed(iface.signal_context()).await.unwrap();

        map
    }

    #[dbus_interface(property)]
    async fn position(&self) -> i64 {
        0
    }

    #[dbus_interface(property)]
    async fn minimum_rate(&self) -> f32 {
        0.0
    }

    #[dbus_interface(property)]
    async fn maximum_rate(&self) -> f32 {
        5.0
    }

    #[dbus_interface(property)]
    async fn can_go_next(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    async fn can_go_previous(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    async fn can_play(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    async fn can_pause(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    async fn can_seek(&self) -> bool {
        false
    }

    #[dbus_interface(property)]
    async fn can_control(&self) -> bool {
        true
    }
}

pub async fn mpris(queue: Arc<Mutex<Queue>>) {
    MprisHandler::new(queue).await
}
