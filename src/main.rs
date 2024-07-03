use std::fmt;
use std::ops::Deref;
use std::sync::atomic::Ordering;
use std::sync::Arc;

mod app;
mod player;
mod queue;
mod song;
mod ui;

use app::{App, Effect};
use mpris_server::Server;

pub enum Message {
    Static(&'static str),
    Dynamic(String),
}

impl Message {
    pub fn new(s: impl ToString) -> Self {
        Self::Dynamic(s.to_string())
    }

    pub fn stc(s: &'static str) -> Self {
        Self::Static(s)
    }
}

impl Deref for Message {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Dynamic(s) => s,
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            match self {
                Self::Static(s) => *s,
                Self::Dynamic(s) => s,
            }
        )
    }
}

#[async_std::main]
async fn main() {
    let pid = std::process::id();
    let server = Arc::new(
        Server::new(
            &format!("com.cramp.instance{pid}"),
            App::new(std::env::args().nth(1)).await,
        )
        .await
        .unwrap_or_else(|e| {
            eprintln!("failed to launch MPRIS server: {e}");
            cod::term::disable_raw_mode();
            cod::term::primary_screen();
            std::process::exit(1);
        }),
    );

    let app = server.imp();
    loop {
        app.poll().await;
        for effect in app.effects.lock().await.drain(..) {
            match effect {
                Effect::Signal(s) => {
                    if let Err(e) = server.emit(s).await {
                        app.add_message(Message::new(format!("dbus error: {e}")))
                            .await;
                    }
                }
                Effect::Changed(c) => {
                    if let Err(e) = server.properties_changed(c).await {
                        app.add_message(Message::new(format!("dbus error: {e}")))
                            .await;
                    }
                }
            }
        }

        if app.quit.load(Ordering::Relaxed) {
            break;
        }
    }
}
