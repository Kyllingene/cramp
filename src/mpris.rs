use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};
use dbus::{blocking::LocalConnection, MethodErr};
use dbus_tree::{Factory, Access};

use crate::Message;

pub struct MprisRecv {
    pub vol: Sender<f64>,
    pub rate: Sender<f64>,
    pub shuf: Sender<bool>,
    pub stat: Sender<&'static str>,


    pub conn: LocalConnection
}

pub fn mpris(tx: Sender<Message>) -> MprisRecv {
    let conn = LocalConnection::new_session().unwrap();
    conn.request_name("org.mpris.MediaPlayer2.cramp", false, true, false)
        .expect("Failed to register dbus name");

    let f = Factory::new_fn::<()>();

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
    let tx_get_stat = tx.clone();
    let (tx_stat, rx_stat) = unbounded();
    let tx_open_uri = tx;

    let tree =
        f.tree(())
            .add(
                f.object_path("/org/mpris/MediaPlayer2", ())
                    .introspectable()
                    .add(
                        f.interface("org.mpris.MediaPlayer2", ())
                            .add_m(f.method("Quit", (), |_| std::process::exit(0)))
                            .add_m(f.method("Raise", (), |m| Ok(vec![m.msg.method_return()])))
                            .add_p(f.property::<bool, _>("CanQuit", ()).on_get(|i, _| {
                                i.append(true);
                                Ok(())
                            }))
                            .add_p(f.property::<bool, _>("CanRaise", ()).on_get(|i, _| {
                                i.append(false);
                                Ok(())
                            }))
                            .add_p(f.property::<bool, _>("HasTrackList", ()).on_get(|i, _| {
                                i.append(false);
                                Ok(())
                            }))
                            .add_p(
                                f.property::<&'static str, _>("Identity", ())
                                    .on_get(|i, _| {
                                        i.append("cramp");
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<Vec<&'static str>, _>("SupportedUriSchemes", ())
                                    .on_get(|i, _| {
                                        i.append(vec!["file"]);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<Vec<&'static str>, _>("SupportedMimeTypes", ())
                                    .on_get(|i, _| {
                                        i.append(vec![
                                            "audio/mpeg",
                                            "audio/ogg",
                                            "audio/wav",
                                            "audio/flac",
                                            "audio/vorbis",
                                        ]);
                                        Ok(())
                                    }),
                            ),
                    )
                    .introspectable()
                    .add(
                        f.interface("org.mpris.MediaPlayer2.Player", ())
                            .add_m(f.method("Next", (), move |m| {
                                tx_next.send(Message::Next).unwrap();
                                Ok(vec![m.msg.method_return()])
                            }))
                            .add_m(f.method("Previous", (), move |m| {
                                tx_prev.send(Message::Prev).unwrap();
                                Ok(vec![m.msg.method_return()])
                            }))
                            .add_m(f.method("Play", (), move |m| {
                                tx_play.send(Message::Play).unwrap();
                                Ok(vec![m.msg.method_return()])
                            }))
                            .add_m(f.method("Pause", (), move |m| {
                                tx_pause.send(Message::Pause).unwrap();
                                Ok(vec![m.msg.method_return()])
                            }))
                            .add_m(f.method("PlayPause", (), move |m| {
                                tx_playpause.send(Message::PlayPause).unwrap();
                                Ok(vec![m.msg.method_return()])
                            }))
                            .add_m(f.method("Stop", (), move |m| {
                                tx_stop.send(Message::Stop).unwrap();
                                Ok(vec![m.msg.method_return()])
                            }))
                            .add_m(
                                f.method("Seek", (), |m| Ok(vec![m.msg.method_return()]))
                                    .inarg::<i32, _>("Offset"),
                            )
                            .add_m(
                                f.method("SetPosition", (), |m| Ok(vec![m.msg.method_return()]))
                                    .inarg::<&str, _>("TrackId")
                                    .inarg::<i64, _>("Position"),
                            )
                            .add_m(
                                f.method("OpenUri", (), move |m| {
                                    tx_open_uri
                                        .send(Message::OpenUri(m.msg.read1::<&str>()?.to_string()))
                                        .map_err(|e| MethodErr::failed(&e))?;

                                    Ok(vec![m.msg.method_return()])
                                })
                                .inarg::<&str, _>("Uri"),
                            )
                            .add_p(f.property::<&str, _>("PlaybackStatus", ()).on_get(
                                move |i, _| {
                                    tx_get_stat
                                        .send(Message::GetStatus)
                                        .map_err(|e| MethodErr::failed(&e))?;

                                    let stat = rx_stat
                                        .recv_timeout(Duration::from_millis(200))
                                        .map_err(|e| MethodErr::failed(&e))?;
                                    i.append(stat);
                                    Ok(())
                                },
                            ))
                            .add_p(
                                f.property::<f64, _>("Rate", ())
                                    .access(Access::ReadWrite)
                                    .on_get(move |i, _| {
                                        tx_get_rate
                                            .send(Message::GetRate)
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        let rate = rx_rate
                                            .recv_timeout(Duration::from_millis(200))
                                            .map_err(|e| MethodErr::failed(&e))?;
                                        i.append(rate);
                                        Ok(())
                                    })
                                    .on_set(move |i, _| {
                                        tx_set_rate
                                            .send(Message::SetRate(i.read()?))
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<f64, _>("Volume", ())
                                    .access(Access::ReadWrite)
                                    .on_get(move |i, _| {
                                        tx_get_vol
                                            .send(Message::GetVolume)
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        let vol = rx_vol
                                            .recv_timeout(Duration::from_millis(200))
                                            .map_err(|e| MethodErr::failed(&e))?;
                                        i.append(vol);
                                        Ok(())
                                    })
                                    .on_set(move |i, _| {
                                        tx_set_vol
                                            .send(Message::SetVolume(i.read()?))
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("Shuffle", ())
                                    .access(Access::ReadWrite)
                                    .on_get(move |i, _| {
                                        tx_get_shuf
                                            .send(Message::GetShuffle)
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        let shuf = rx_shuf
                                            .recv_timeout(Duration::from_millis(200))
                                            .map_err(|e| MethodErr::failed(&e))?;
                                        i.append(shuf);
                                        Ok(())
                                    })
                                    .on_set(move |_, _| {
                                        tx_set_shuf
                                            .send(Message::Shuffle)
                                            .map_err(|e| MethodErr::failed(&e))?;

                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<i64, _>("Position", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(0i64);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<f64, _>("MinimumRate", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(1.0);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<f64, _>("MaximumRate", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(1.0);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanGoNext", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanGoPrevious", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanPlay", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanPause", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanSeek", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(false);
                                        Ok(())
                                    }),
                            )
                            .add_p(
                                f.property::<bool, _>("CanControl", ())
                                    .access(Access::Read)
                                    .on_get(|i, _| {
                                        i.append(true);
                                        Ok(())
                                    }),
                            ),
                    )
                    .introspectable(),
            )
            .add(f.object_path("/", ()).introspectable());

    tree.start_receive(&conn);

    MprisRecv{
        vol: tx_vol,
        rate: tx_rate,
        shuf: tx_shuf,
        stat: tx_stat,
        
        conn,
    }
}