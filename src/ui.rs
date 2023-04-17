use std::mem;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Layout};
use eframe::emath::Align;
use eframe::App;
use rayon::prelude::*;

use crate::queue::LoopMode;
use crate::{queue::Queue, song::Song, Message};

pub fn ui(queue: Arc<Mutex<Queue>>, tx: Sender<Message>, playlist: Option<PathBuf>) {
    eframe::run_native(
        "CRAMP",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(Player::new(queue, tx, playlist))),
    )
    .unwrap();
}

struct Player {
    queue: Arc<Mutex<Queue>>,
    tx: Sender<Message>,

    search: String,
    results: Vec<Song>,

    adding: bool,
    to_add: String,
    to_add_next: String,
    to_add_noshuffle: bool,

    save_dialog: Option<egui_file::FileDialog>,
    playlist: Option<PathBuf>,
}

impl Player {
    pub fn new(queue: Arc<Mutex<Queue>>, tx: Sender<Message>, playlist: Option<PathBuf>) -> Self {
        let results = queue.lock().unwrap().songs.clone();
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
                                queue.play_pause();
                            }

                            if ui.button("Next").clicked() {
                                queue.next();
                            }

                            if ui.button("Previous").clicked() {
                                queue.last();
                            }

                            if ui.button("Exit").clicked() {
                                self.tx.send(Message::Exit).unwrap();
                                exit(0);
                            }

                            if ui.text_edit_singleline(&mut self.search).changed() {
                                if self.search.is_empty() {
                                    self.results = queue.songs.clone();
                                } else {
                                    self.results = queue
                                        .songs
                                        .par_iter()
                                        .filter(|s| {
                                            s.name.to_lowercase().contains(&self.search.to_lowercase())
                                                || s.file.contains(&self.search.to_lowercase())
                                        })
                                        .cloned()
                                        .collect();
                                }
                            }

                            let text_style = egui::TextStyle::Body;
                            let row_height = ui.text_style_height(&text_style) * 5.5;
                            egui::ScrollArea::vertical().id_source("songs").show_rows(
                                ui,
                                row_height,
                                self.results.len(),
                                |ui, range| {
                                    for i in range {
                                        let song = self.results[i].clone();
                                        ui.group(|ui| {
                                            ui.label(&song.name);

                                            if let Some(next) = &song.next {
                                                ui.label(format!("Next:\n{next}"));
                                            }

                                            if ui.button("Remove").clicked() {
                                                if let Some(i) = queue
                                                    .songs
                                                    .iter()
                                                    .enumerate()
                                                    .find(|(_, s)| s == &&song)
                                                    .map(|(i, _)| i)
                                                {
                                                    queue.songs.remove(i);
                                                }
                                                self.results.remove(i);
                                            } else if ui.button("Play").clicked() {
                                                queue.stop();
                                                if let Some(next) = &song.next {
                                                    if let Some(song) =
                                                        queue.songs.iter().find(|s| &s.file == next)
                                                    {
                                                        queue.next = song.clone().into();
                                                    }
                                                }
                                                queue.current = Some(song);
                                                queue.play();
                                            } else if ui.button("Next").clicked() {
                                                queue.next = song.into();
                                            } else if ui.button("Queue").clicked() {
                                                queue.queue(song.id);
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
                                        let next = if self.to_add_next.is_empty() {
                                            None
                                        } else {
                                            Some(mem::take(&mut self.to_add_next))
                                        };

                                        let song = Song::new(add, None, next, None)
                                            .noshuffle(self.to_add_noshuffle);

                                        if song.name.contains(&self.search) {
                                            self.results.push(song.clone());
                                        }

                                        queue.songs.push(song);
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
