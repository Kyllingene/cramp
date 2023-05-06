use std::time::Duration;

use crossbeam_channel::{unbounded, Sender};
use dbus::{arg::PropMap, blocking::Connection, MethodErr};
use dbus_crossroads::{Context, Crossroads};

use crate::Message;

pub struct MprisRecv {
    pub vol: Sender<f64>,
    pub rate: Sender<f64>,
    pub shuf: Sender<bool>,
    pub stat: Sender<&'static str>,
    pub meta: Sender<PropMap>,
    pub loop_mode: Sender<String>,
}

pub fn mpris(tx: Sender<Message>) -> MprisRecv {
    let conn = Connection::new_session().unwrap();

    let name = format!(
        "org.mpris.MediaPlayer2.cramp.instance{}",
        std::process::id()
    );
    conn.request_name(name, false, true, false)
        .expect("Failed to register dbus name");

    let tx_exit = tx.clone();
    let tx_next = tx.clone();
    let tx_prev = tx.clone();
    let tx_play = tx.clone();
    let tx_pause = tx.clone();
    let tx_playpause = tx.clone();
    let tx_stop = tx.clone();

    let tx_get_vol = tx.clone();
    let tx_set_vol = tx.clone();
    let (tx_vol, rx_vol) = unbounded();
    let tx_get_rate = tx.clone();
    let tx_set_rate = tx.clone();
    let (tx_rate, rx_rate) = unbounded();
    let tx_set_shuf = tx.clone();
    let tx_get_shuf = tx.clone();
    let (tx_shuf, rx_shuf) = unbounded();
    let tx_set_loop_mode = tx.clone();
    let tx_get_loop_mode = tx.clone();
    let (tx_loop_mode, rx_loop_mode) = unbounded();
    let tx_get_stat = tx.clone();
    let (tx_stat, rx_stat) = unbounded();
    let tx_open_uri = tx.clone();
    let tx_get_meta = tx;
    let (tx_meta, rx_meta) = unbounded();

    let mut cr = Crossroads::new();

    let root_iface = cr.register("org.mpris.MediaPlayer2", |b| {
        b.property("CanQuit")
            .emits_changed_const()
            .get(|_, _| Ok(true));

        b.property("Fullscreen")
            .emits_changed_const()
            .get(|_, _| -> Result<bool, MethodErr> { Err(MethodErr::no_property("Fullscreen")) });

        b.property("CanSetFullscreen").get(|_, _| Ok(false));

        b.property("CanRaise")
            .emits_changed_const()
            .get(|_, _| Ok(true));

        b.property("HasTrackList")
            .emits_changed_const()
            .get(|_, _| Ok(true));

        b.property("Identity")
            .emits_changed_const()
            .get(|_, _| Ok((String::from("CRAMP"),)));

        b.property("DesktopEntry")
            .emits_changed_const()
            .get(|_, _| -> Result<bool, MethodErr> { Err(MethodErr::no_property("DesktopEntry")) });

        b.property("SupportedUriSchemes")
            .emits_changed_const()
            .get(|_, _| Ok(vec!["file".to_string()]));

        b.property("SupportedMimeTypes")
            .emits_changed_const()
            .get(|_, _| {
                Ok((vec![
                    "audio/mpeg".to_string(),
                    "audio/ogg".to_string(),
                    "audio/wav".to_string(),
                    "audio/flac".to_string(),
                    "audio/vorbis".to_string(),
                ],))
            });

        b.method(
            "Quit",
            (),
            (),
            move |_: &mut Context, _, _: ()| -> Result<(), MethodErr> {
                tx_exit.send(Message::Exit).unwrap();
                std::process::exit(0);
            },
        );

        b.method(
            "Raise",
            (),
            (),
            move |_: &mut Context, _, _: ()| -> Result<(), MethodErr> {
                Err(MethodErr::no_method("Raise"))
            },
        );
    });

    let player_iface = cr.register("org.mpris.MediaPlayer2.Player", |b| {
        b.method("Next", (), (), move |_, _, _: ()| {
            tx_next.send(Message::Next).unwrap();
            Ok(())
        });

        b.method("Previous", (), (), move |_, _, _: ()| {
            tx_prev.send(Message::Prev).unwrap();
            Ok(())
        });

        b.method("Play", (), (), move |_, _, _: ()| {
            tx_play.send(Message::Play).unwrap();
            Ok(())
        });

        b.method("Pause", (), (), move |_, _, _: ()| {
            tx_pause.send(Message::Pause).unwrap();
            Ok(())
        });

        b.method("PlayPause", (), (), move |_, _, _: ()| {
            tx_playpause.send(Message::PlayPause).unwrap();
            Ok(())
        });

        b.method("Stop", (), (), move |_, _, _: ()| {
            tx_stop.send(Message::Stop).unwrap();
            Ok(())
        });

        b.method("Seek", ("Offset",), (), |_, _, _: (i64,)| Ok(()));

        b.method(
            "SetPosition",
            ("TrackId", "Position"),
            (),
            |_, _, _: (String, i64)| Ok(()),
        );

        b.method("OpenUri", ("Uri",), (), move |ctx, _, _: (String,)| {
            tx_open_uri
                .send(Message::OpenUri(ctx.message().read1::<&str>()?.to_string()))
                .map_err(|e| MethodErr::failed(&e))?;

            Ok(())
        });

        b.property("PlaybackStatus").get(move |_, _| {
            tx_get_stat
                .send(Message::GetStatus)
                .map_err(|e| MethodErr::failed(&e))?;

            let stat: &'static str = rx_stat
                .recv_timeout(Duration::from_millis(200))
                .map_err(|e| MethodErr::failed(&e))?;

            Ok(stat.to_string())
        });

        b.property("LoopStatus")
            .get(move |_, _| {
                tx_get_loop_mode
                    .send(Message::GetLoop)
                    .map_err(|e| MethodErr::failed(&e))?;

                let loop_mode: String = rx_loop_mode
                    .recv_timeout(Duration::from_millis(200))
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(loop_mode)
            })
            .set(move |_, _, loop_mode| {
                tx_set_loop_mode
                    .send(Message::SetLoop(loop_mode.clone()))
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(Some(loop_mode))
            });

        b.property("Rate")
            .get(move |_, _| {
                tx_get_rate
                    .send(Message::GetRate)
                    .map_err(|e| MethodErr::failed(&e))?;

                let rate = rx_rate
                    .recv_timeout(Duration::from_millis(200))
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(rate)
            })
            .set(move |_, _, rate| {
                tx_set_rate
                    .send(Message::SetRate(rate))
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(Some(rate))
            });

        b.property("Volume")
            .get(move |_, _| {
                tx_get_vol
                    .send(Message::GetVolume)
                    .map_err(|e| MethodErr::failed(&e))?;

                let vol = rx_vol
                    .recv_timeout(Duration::from_millis(200))
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(vol)
            })
            .set(move |_, _, vol| {
                tx_set_vol
                    .send(Message::SetVolume(vol))
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(Some(vol))
            });

        b.property("Shuffle")
            .get(move |_, _| {
                tx_get_shuf
                    .send(Message::GetShuffle)
                    .map_err(|e| MethodErr::failed(&e))?;

                let shuf = rx_shuf
                    .recv_timeout(Duration::from_millis(200))
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(shuf)
            })
            .set(move |_, _, shuf| {
                tx_set_shuf
                    .send(Message::Shuffle)
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(Some(shuf))
            });

        b.property("Metadata").get(move |_, _| {
            tx_get_meta
                .send(Message::GetMetadata)
                .map_err(|e| MethodErr::failed(&e))?;

            let meta = rx_meta
                .recv_timeout(Duration::from_millis(200))
                .map_err(|e| MethodErr::failed(&e))?;

            Ok(meta)
        });

        b.property("Position").get(|_, _| Ok(0i64));

        b.property("MinimumRate").get(|_, _| Ok(0f64));

        b.property("MaximumRate").get(|_, _| Ok(5f64));

        b.property("CanGoNext").get(|_, _| Ok(true));

        b.property("CanGoPrevious").get(|_, _| Ok(true));

        b.property("CanPlay").get(|_, _| Ok(true));

        b.property("CanPause").get(|_, _| Ok(true));

        b.property("CanSeek").get(|_, _| Ok(false));

        b.property("CanControl").get(|_, _| Ok(true));
    });

    let props_iface = cr.properties();

    cr.insert(
        "/org/mpris/MediaPlayer2",
        &[root_iface, player_iface, props_iface],
        (),
    );

    std::thread::spawn(move || cr.serve(&conn).unwrap());

    MprisRecv {
        vol: tx_vol,
        rate: tx_rate,
        shuf: tx_shuf,
        stat: tx_stat,
        meta: tx_meta,
        loop_mode: tx_loop_mode,
    }
}
