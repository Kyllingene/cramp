// use std::sync::mpsc::Sender;
use crossbeam_channel::Sender;

use windows::Foundation::TypedEventHandler;
use windows::Media::Playback::MediaPlayer;
use windows::Media::SystemMediaTransportControls as SMTC;
use windows::Media::SystemMediaTransportControlsButtonPressedEventArgs as SMTCBPEA;

use crate::Message;

pub struct Controller {
    pub ctrl: SMTC,

    _player: MediaPlayer,
    _handler: TypedEventHandler<SMTC, SMTCBPEA>,
}

pub fn controls(tx: Sender<Message>) -> Controller {
    let mut failed = 0;
    let player = loop {
        match MediaPlayer::new() {
            Err(e) => {
                if failed >= 3 {
                    panic!("{}", e);
                }

                failed += 1;
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            Ok(c) => {
                break c;
            }
        }
    };

    let ctrl = player.SystemMediaTransportControls().unwrap();

    let handler = TypedEventHandler::new(move |_, b: &Option<SMTCBPEA>| {
        if let Some(b) = b {
            if let Ok(b) = b.Button() {
                match b.0 {
                    0 => {
                        tx.send(Message::PlayPause).unwrap();
                    }
                    1 => {
                        tx.send(Message::PlayPause).unwrap();
                    }
                    2 => {
                        tx.send(Message::Stop).unwrap();
                    }
                    6 => {
                        tx.send(Message::Next).unwrap();
                    }
                    7 => {
                        tx.send(Message::Prev).unwrap();
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    });
    ctrl.ButtonPressed(&handler).unwrap();

    ctrl.SetIsPlayEnabled(true).unwrap();
    ctrl.SetIsPauseEnabled(true).unwrap();
    ctrl.SetIsNextEnabled(true).unwrap();
    ctrl.SetIsPreviousEnabled(true).unwrap();

    Controller {
        ctrl,
        _player: player,
        _handler: handler,
    }
}
