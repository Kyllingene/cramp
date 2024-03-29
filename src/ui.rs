use crossbeam_channel::Sender;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Layout};
use eframe::emath::Align;
use eframe::App;
use rayon::prelude::*;

use crate::queue::LoopMode;
use crate::{queue::Queue, song::Song, Message};

const PERSIST_FILENAME: &str = ".cramp-playlist.m3u";

pub fn ui(queue: Arc<Mutex<Queue>>, tx: Sender<Message>, playlist: Option<PathBuf>) {
    eframe::run_native(
        "CRAMP",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(Player::new(queue, tx, playlist))),
    )
    .unwrap();
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Result {
    song: Song,
    selected: bool,
    set_next: bool,
}

impl Result {
    fn new(song: Song) -> Self {
        Self {
            song,
            selected: false,
            set_next: false,
        }
    }
}

struct Player {
    queue: Arc<Mutex<Queue>>,
    tx: Sender<Message>,

    search: String,
    results: Vec<Result>,

    adding: bool,
    to_add: String,
    to_add_next: String,
    to_add_noshuffle: bool,

    save_dialog: Option<egui_file::FileDialog>,
    playlist: Option<PathBuf>,
}

impl Player {
    pub fn new(queue: Arc<Mutex<Queue>>, tx: Sender<Message>, playlist: Option<PathBuf>) -> Self {
        let results = queue
            .lock()
            .unwrap()
            .songs
            .clone()
            .into_iter()
            .map(Result::new)
            .collect();

        Self {
            queue,
            tx,
            search: String::new(),
            results,

            adding: false,
            to_add: String::new(),
            to_add_next: String::new(),
            to_add_noshuffle: false,

            save_dialog: None,
            playlist,
        }
    }
}

impl App for Player {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(
                Layout::default()
                    .with_cross_align(Align::Center)
                    .with_main_align(Align::Center),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_source("mainview")
                        .show(ui, |ui| {
                            ui.heading("CRAMP");

                            let mut queue = self.queue.lock().unwrap();

                            ui.label("Current");
                            ui.label(
                                queue
                                    .current
                                    .as_ref()
                                    .map(|s| s.name.clone())
                                    .unwrap_or_else(|| String::from("None")),
                            );

                            ui.label("Next");
                            ui.label(
                                queue
                                    .next
                                    .as_ref()
                                    .map(|s| s.song.name.clone())
                                    .unwrap_or_else(|| String::from("None")),
                            );

                            if ui.button("Skip next").clicked() {
                                queue.skip_next();
                            }

                            ui.label("Volume");
                            let mut vol = queue.volume();
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=5.0)).changed() {
                                queue.set_volume(vol);
                            }

                            ui.label("Speed");
                            let mut speed = queue.speed();
                            if ui.add(egui::Slider::new(&mut speed, 0.01..=5.0)).changed() {
                                queue.set_speed(speed);
                            }

                            ui.label("Silence length");
                            let mut start = *queue.silence.start();
                            let mut end = *queue.silence.end();
                            if ui
                                .add(egui::Slider::new(&mut start, 0.0..=3600.0))
                                .changed()
                            {
                                queue.silence = start..=end;
                            }
                            if ui.add(egui::Slider::new(&mut end, 0.0..=3600.0)).changed() {
                                queue.silence = start..=end;
                            }

                            if ui
                                .button(if queue.shuffle {
                                    "Unshuffle"
                                } else {
                                    "Shuffle"
                                })
                                .clicked()
                            {
                                queue.shuffle();
                            }

                            if ui
                                .button(match queue.loop_mode {
                                    LoopMode::None => "Not looping",
                                    LoopMode::Playlist => "Looping by playlist",
                                    LoopMode::Track => "Looping by track",
                                })
                                .clicked()
                            {
                                queue.loop_mode += 1;
                            }

                            ui.label(if queue.paused() { "Paused" } else { "Playing" });

                            if ui
                                .button(if queue.paused() { "Play" } else { "Pause" })
                                .clicked()
                            {
                                // queue.play_pause();
                                self.tx.send(Message::PlayPause).unwrap();
                            }

                            if ui.button("Next").clicked() {
                                // queue.next();
                                self.tx.send(Message::Next).unwrap();
                            }

                            if ui.button("Previous").clicked() {
                                // queue.last();
                                self.tx.send(Message::Prev).unwrap();
                            }

                            if ui.button("Clear playlist").clicked() {
                                queue.songs.clear();
                                queue.queue.clear();
                            }

                            if ui.button("Exit").clicked() {
                                self.tx.send(Message::Exit).unwrap();

                                match dirs::home_dir() {
                                    Some(mut path) => {
                                        path.push(PERSIST_FILENAME);
                                        queue.save_playlist(path);
                                    }
                                    None => eprintln!("Failed to persist playlist"),
                                }

                                exit(0);
                            }

                            let text_style = egui::TextStyle::Body;
                            let row_height = ui.text_style_height(&text_style) * 3.0;
                            egui::ScrollArea::vertical()
                                .id_source("queue")
                                .max_height(100.0)
                                .show_rows(ui, row_height, queue.user_queue.len(), |ui, range| {
                                    for i in range {
                                        let brk = ui
                                            .group(|ui| {
                                                let song = &queue.user_queue[i];
                                                ui.label(&song.name);

                                                if let Some(micros) = song.length {
                                                    ui.label(format!(
                                                        "{}:{}",
                                                        micros / 60000000,
                                                        (micros / 1000000) % 60,
                                                    ));
                                                }

                                                if let Some(next) = &song.next {
                                                    ui.label(format!("Next: {}", next));
                                                }

                                                if ui.button("Remove").clicked() {
                                                    queue.user_queue.remove(i);
                                                    return true;
                                                }

                                                false
                                            })
                                            .inner;
                                        if brk {
                                            break;
                                        }
                                    }
                                });

                            if ui.button("Remove").clicked() {
                                let ids = self.results.iter_mut().filter_map(|song| {
                                    if song.selected {
                                        song.selected = false;
                                        Some(song.song.id)
                                    } else {
                                        None
                                    }
                                });

                                for id in ids {
                                    queue.remove(id);
                                }

                                self.search.clear();
                            } else if ui.button("Play").clicked() {
                                if let Some(result) = self.results.iter_mut().find(|r| r.selected) {
                                    let song = result.song.clone();
                                    result.selected = false;
                                    // queue.stop();
                                    self.tx.send(Message::Stop).unwrap();
                                    if let Some(next) = &song.next {
                                        if let Some(song) =
                                            queue.songs.iter().find(|s| &s.file == next)
                                        {
                                            queue.next = song.clone().into();
                                        }
                                    }
                                    queue.current = Some(song);
                                    // queue.play();
                                    self.tx.send(Message::Play).unwrap();
                                }
                            } else if ui.button("Next").clicked() {
                                if let Some(result) = self.results.iter_mut().find(|r| r.selected) {
                                    if let Some(next) = queue.next.take() {
                                        queue.queue.push_front(next.song);
                                    }
                                    result.selected = false;
                                    queue.next = result.song.clone().into();
                                }
                            } else if ui.button("Queue").clicked() {
                                for result in self.results.iter_mut() {
                                    if result.selected {
                                        result.selected = false;
                                        queue.queue(result.song.clone());
                                    }
                                }
                            } else if ui.button("Set Next").clicked() {
                                if let Some(next) =
                                    self.results.iter_mut().find(|result| result.set_next)
                                {
                                    next.set_next = false;

                                    let next = next.song.file.clone();
                                    for result in self.results.iter_mut() {
                                        if result.selected {
                                            result.song.next = Some(next.clone());
                                            result.selected = false;
                                        }
                                    }
                                }
                            }

                            if ui.text_edit_singleline(&mut self.search).changed() {
                                if self.search.is_empty() {
                                    self.results =
                                        queue.songs.clone().into_iter().map(Result::new).collect();
                                } else {
                                    self.results = queue
                                        .songs
                                        .par_iter()
                                        .filter(|s| {
                                            s.name
                                                .to_lowercase()
                                                .contains(&self.search.to_lowercase())
                                                || s.file
                                                    .to_lowercase()
                                                    .contains(&self.search.to_lowercase())
                                        })
                                        .cloned()
                                        .map(Result::new)
                                        .collect();
                                }
                            }

                            let row_height = ui.text_style_height(&text_style) * 2.0;
                            egui::ScrollArea::vertical().id_source("songs").show_rows(
                                ui,
                                row_height,
                                self.results.len(),
                                |ui, range| {
                                    for i in range {
                                        let result = &mut self.results[i];
                                        let song = &result.song;
                                        ui.horizontal(|ui| {
                                            ui.toggle_value(&mut result.set_next, "X");
                                            ui.toggle_value(&mut result.selected, &song.name);

                                            if let Some(micros) = song.length {
                                                ui.label(format!(
                                                    "{}:{}",
                                                    micros / 60000000,
                                                    (micros / 1000000) % 60,
                                                ));
                                            }

                                            if let Some(next) = &song.next {
                                                ui.label(format!("Next: {next}"));
                                            }
                                        });
                                    }
                                },
                            );

                            if !self.adding {
                                if ui.button("Add").clicked() {
                                    self.adding = true;
                                    self.to_add_noshuffle = false;
                                }
                            } else {
                                ui.text_edit_singleline(&mut self.to_add);

                                ui.checkbox(&mut self.to_add_noshuffle, "Dont autoplay in shuffle");

                                ui.label("Next");
                                ui.text_edit_singleline(&mut self.to_add_next);

                                if ui.button("Add").clicked() {
                                    self.adding = false;

                                    let add = mem::take(&mut self.to_add);

                                    if Path::new(&add).is_file() {
                                        if add.ends_with(".m3u") || add.ends_with(".m3u8") {
                                            let mut nq = Queue::load(add);
                                            queue.songs.append(&mut nq.songs);
                                        } else {
                                            let next = if self.to_add_next.is_empty() {
                                                None
                                            } else {
                                                Some(mem::take(&mut self.to_add_next))
                                            };

                                            let song = Song::new(add, None, next, None)
                                                .noshuffle(self.to_add_noshuffle);

                                            if song.name.contains(&self.search) {
                                                self.results.push(Result::new(song.clone()));
                                            }

                                            queue.songs.push(song);
                                        }
                                    } else if Path::new(&add).is_dir() {
                                        let mut nq = Queue::load_dir(add);
                                        queue.songs.append(&mut nq.songs);
                                    }
                                }
                            }

                            if ui.button("Save Playlist").clicked() {
                                let mut dialog =
                                    egui_file::FileDialog::save_file(self.playlist.clone());
                                dialog.open();
                                self.save_dialog = Some(dialog);
                            }

                            if let Some(dialog) = &mut self.save_dialog {
                                if dialog.show(ctx).selected() {
                                    if let Some(file) = dialog.path() {
                                        self.playlist = Some(file.clone());
                                        queue.save_playlist(file);
                                    }
                                }
                            }
                        });
                },
            );
        });
    }
}
