use std::{
    process::exit,
    sync::{Arc, Mutex},
};

use crossbeam_channel::Sender;
use eframe::{egui, App};
use rayon::prelude::*;

use crate::{queue::Queue, song::Song, Message};

pub fn ui(queue: Arc<Mutex<Queue>>, tx: Sender<Message>) {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(480.0, 360.0)),
        ..Default::default()
    };

    eframe::run_native(
        "CRAMP",
        options,
        Box::new(|_cc| Box::new(Player::new(queue, tx))),
    )
    .unwrap();
}

struct Player {
    queue: Arc<Mutex<Queue>>,
    tx: Sender<Message>,

    search: String,
    results: Vec<Song>,
}

impl Player {
    pub fn new(queue: Arc<Mutex<Queue>>, tx: Sender<Message>) -> Self {
        let results = queue.lock().unwrap().songs.clone();
        Self {
            queue,
            tx,
            search: String::new(),
            results,
        }
    }
}

impl App for Player {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
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

            egui::ScrollArea::horizontal()
                .id_source("controls")
                .show(ui, |ui| {
                    if ui.button("Toggle Shuffle").clicked() {
                        queue.shuffle();
                    }

                    ui.label(if queue.paused() { "Paused" } else { "Playing" });

                    if ui.button("Play/Pause").clicked() {
                        queue.play_pause();
                    }

                    if ui.button("Next").clicked() {
                        queue.next();
                    }

                    if ui.button("Previous").clicked() {
                        queue.last();
                    }
                });

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
                        .filter(|s| s.name.contains(&self.search) || s.file.contains(&self.search))
                        .cloned()
                        .collect();
                }
            }

            let text_style = egui::TextStyle::Body;
            let row_height = ui.text_style_height(&text_style) * 3.0;
            egui::ScrollArea::vertical().id_source("songs").show_rows(
                ui,
                row_height,
                self.results.len(),
                |ui, range| {
                    for i in range {
                        let song = self.results[i].clone();
                        ui.group(|ui| {
                            ui.label(&song.name);

                            if ui.button("Play").clicked() {
                                queue.stop();
                                queue.current = Some(song);
                                queue.play();
                            } else if ui.button("Next").clicked() {
                                queue.next = song.into();
                            }
                        });
                    }
                },
            );
        });
    }
}
