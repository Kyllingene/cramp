use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use zbus::zvariant::OwnedValue;
use zbus::{dbus_interface, ConnectionBuilder, InterfaceRef};

use crate::Message;

pub struct MprisRecv {
    pub vol: Sender<f64>,
    pub rate: Sender<f64>,
    pub shuf: Sender<bool>,
    pub stat: Sender<&'static str>,
    pub meta: Sender<HashMap<String, OwnedValue>>,
    pub loop_mode: Sender<String>,
}

pub struct MprisHandler {
    tx_exit: Sender<Message>,
}

pub struct Player {
    rx_vol: Receiver<f64>,
    rx_rate: Receiver<f64>,
    rx_shuf: Receiver<bool>,
    rx_stat: Receiver<&'static str>,
    rx_meta: Receiver<HashMap<String, OwnedValue>>,
    rx_loop_mode: Receiver<String>,

    tx_next: Sender<Message>,
    tx_prev: Sender<Message>,
    tx_play: Sender<Message>,
    tx_pause: Sender<Message>,
    tx_playpause: Sender<Message>,
    tx_stop: Sender<Message>,

    tx_open_uri: Sender<Message>,
    tx_get_stat: Sender<Message>,
    tx_get_loop_mode: Sender<Message>,
    tx_set_loop_mode: Sender<Message>,
    tx_get_rate: Sender<Message>,
    tx_set_rate: Sender<Message>,
    tx_get_vol: Sender<Message>,
    tx_set_vol: Sender<Message>,
    tx_get_shuf: Sender<Message>,
    tx_set_shuf: Sender<Message>,
    tx_get_meta: Sender<Message>,
}

impl MprisHandler {
    pub async fn new(tx: Sender<Message>) -> (Arc<InterfaceRef<Player>>, MprisRecv) {
        let (tx_vol, rx_vol) = unbounded();
        let (tx_rate, rx_rate) = unbounded();
        let (tx_shuf, rx_shuf) = unbounded();
        let (tx_stat, rx_stat) = unbounded();
        let (tx_meta, rx_meta) = unbounded();
        let (tx_loop_mode, rx_loop_mode) = unbounded();

        let me = Self {
            tx_exit: tx.clone(),
        };

        let player = Player {
            rx_vol,
            rx_rate,
            rx_shuf,
            rx_stat,
            rx_meta,
            rx_loop_mode,

            tx_next: tx.clone(),
            tx_prev: tx.clone(),
            tx_play: tx.clone(),
            tx_pause: tx.clone(),
            tx_playpause: tx.clone(),
            tx_stop: tx.clone(),

            tx_open_uri: tx.clone(),
            tx_get_stat: tx.clone(),
            tx_get_loop_mode: tx.clone(),
            tx_set_loop_mode: tx.clone(),
            tx_get_rate: tx.clone(),
            tx_set_rate: tx.clone(),
            tx_get_vol: tx.clone(),
            tx_set_vol: tx.clone(),
            tx_get_shuf: tx.clone(),
            tx_set_shuf: tx.clone(),
            tx_get_meta: tx,
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

        let iface_ref = Arc::new(
            connection
                .object_server()
                .interface::<_, Player>("/org/mpris/MediaPlayer2")
                .await
                .unwrap(),
        );

        tokio::spawn(async move { loop {} });

        let recv = MprisRecv {
            vol: tx_vol,
            rate: tx_rate,
            shuf: tx_shuf,
            stat: tx_stat,
            meta: tx_meta,
            loop_mode: tx_loop_mode,
        };

        (iface_ref, recv)
    }
}

#[dbus_interface(name = "org.mpris.MediaPlayer2")]
impl MprisHandler {
    async fn quit(&self) {
        self.tx_exit.send(Message::Exit).unwrap();
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
        self.tx_next.send(Message::Next).unwrap();
    }

    async fn previous(&self) {
        self.tx_prev.send(Message::Prev).unwrap();
    }

    async fn play(&self) {
        self.tx_play.send(Message::Play).unwrap();
    }

    async fn pause(&self) {
        self.tx_pause.send(Message::Pause).unwrap();
    }

    async fn play_pause(&self) {
        self.tx_playpause.send(Message::PlayPause).unwrap();
    }

    async fn stop(&self) {
        self.tx_stop.send(Message::Stop).unwrap();
    }

    async fn seek(&self, _offset: i64) {}

    async fn set_position(&self, _track_id: String, _position: i64) {}

    async fn open_uri(&self, uri: String) {
        self.tx_open_uri.send(Message::OpenUri(uri)).unwrap();
    }

    #[dbus_interface(property)]
    async fn playback_status(&self) -> &str {
        self.tx_get_stat.send(Message::GetStatus).unwrap();

        let stat = self
            .rx_stat
            .recv_timeout(Duration::from_millis(200))
            .unwrap();

        stat
    }

    // b.property("LoopStatus")
    #[dbus_interface(property)]
    async fn loop_status(&self) -> String {
        self.tx_get_loop_mode.send(Message::GetLoop).unwrap();

        let loop_mode = self
            .rx_loop_mode
            .recv_timeout(Duration::from_millis(200))
            .unwrap();

        loop_mode
    }
    #[dbus_interface(property)]
    async fn set_loop_status(&mut self, loop_mode: String) {
        self.tx_set_loop_mode
            .send(Message::SetLoop(loop_mode))
            .unwrap();
    }

    #[dbus_interface(property)]
    async fn rate(&self) -> u64 {
        self.tx_get_rate.send(Message::GetRate).unwrap();

        let rate = self
            .rx_rate
            .recv_timeout(Duration::from_millis(200))
            .unwrap();

        rate as u64
    }

    #[dbus_interface(property)]
    async fn set_rate(&mut self, rate: u64) {
        self.tx_set_rate
            .send(Message::SetRate(rate as f64))
            .unwrap();
    }

    #[dbus_interface(property)]
    async fn volume(&self) -> f64 {
        self.tx_get_vol.send(Message::GetVolume).unwrap();

        let vol = self
            .rx_vol
            .recv_timeout(Duration::from_millis(200))
            .unwrap();

        vol
    }

    #[dbus_interface(property)]
    async fn set_volume(&mut self, volume: f64) {
        self.tx_set_vol.send(Message::SetVolume(volume)).unwrap();
    }

    #[dbus_interface(property)]
    async fn shuffle(&self) -> bool {
        self.tx_get_shuf.send(Message::GetShuffle).unwrap();

        let shuf = self
            .rx_shuf
            .recv_timeout(Duration::from_millis(200))
            .unwrap();

        shuf
    }

    #[dbus_interface(property)]
    async fn set_shuffle(&mut self, _shuf: bool) {
        self.tx_set_shuf.send(Message::Shuffle).unwrap();
    }

    #[dbus_interface(property)]
    async fn metadata(&self) -> HashMap<String, OwnedValue> {
        self.tx_get_meta.send(Message::GetMetadata).unwrap();

        let meta = self
            .rx_meta
            .recv_timeout(Duration::from_millis(200))
            .unwrap();

        meta
    }

    #[dbus_interface(property)]
    async fn position(&self) -> i64 {
        0
    }

    #[dbus_interface(property)]
    async fn minimum_rate(&self) -> f64 {
        0.0
    }

    #[dbus_interface(property)]
    async fn maximum_rate(&self) -> f64 {
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

pub async fn mpris(tx: Sender<Message>) -> (Arc<InterfaceRef<Player>>, MprisRecv) {
    MprisHandler::new(tx).await
}
