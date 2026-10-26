#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod engine;
use eframe::egui;
use engine::{Score, TestFile, WORKLOADS};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

enum Event {
    Status(String),
    Score(usize, usize, Score),
    Done(Result<(), String>),
}
struct App {
    directory: PathBuf,
    size: u64,
    repeats: usize,
    seconds: u64,
    unit: usize,
    scores: [[Option<Score>; 2]; 4],
    status: String,
    receiver: Option<mpsc::Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Default for App {
    fn default() -> Self {
        Self {
            directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("C:\\")),
            size: 1024,
            repeats: 3,
            seconds: 5,
            unit: 0,
            scores: [[None; 2]; 4],
            status: "Ready".into(),
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }
}
impl App {
    fn start(&mut self, row: Option<usize>, ctx: egui::Context) {
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let directory = self.directory.clone();
        let size = self.size * 1048576;
        let repeats = self.repeats;
        let seconds = self.seconds;
        for i in 0..4 {
            if row.is_none_or(|r| r == i) {
                self.scores[i] = [None; 2];
            }
        }
        self.worker = Some(std::thread::spawn(move || {
            let send = |event| {
                let _ = tx.send(event);
                ctx.request_repaint();
            };
            let result = (|| -> Result<(), String> {
                send(Event::Status("Preparing test file…".into()));
                let file =
                    TestFile::create(&directory, size, &cancel).map_err(|e| e.to_string())?;
                for column in 0..2 {
                    for (i, workload) in WORKLOADS.iter().enumerate() {
                        if row.is_some_and(|r| r != i) {
                            continue;
                        }
                        let mut best = Score::default();
                        for repeat in 0..=repeats {
                            if cancel.load(Ordering::Relaxed) {
                                return Err("Cancelled".into());
                            }
                            send(Event::Status(format!(
                                "{} · {} · {}",
                                workload.name,
                                if column == 0 { "Read" } else { "Write" },
                                if repeat == 0 {
                                    "Warm-up".into()
                                } else {
                                    format!("{repeat}/{repeats}")
                                }
                            )));
                            let score = file
                                .measure(
                                    *workload,
                                    column == 1,
                                    Duration::from_secs(seconds),
                                    &cancel,
                                )
                                .map_err(|e| e.to_string())?;
                            if cancel.load(Ordering::Relaxed) {
                                return Err("Cancelled".into());
                            }
                            if repeat > 0 && score.mbps > best.mbps {
                                best = score;
                                send(Event::Score(i, column, best));
                            }
                        }
                    }
                }
                Ok(())
            })();
            send(Event::Done(result));
        }));
    }
    fn report(&self) -> String {
        let mut text = format!(
            "Obsidian Disk 0.1.0\n{} · {} MiB · {} runs · {} s\n",
            self.directory.display(),
            self.size,
            self.repeats,
            self.seconds
        );
        for (i, workload) in WORKLOADS.iter().enumerate() {
            text.push_str(&format!("{}\n", workload.name));
            for (j, name) in ["Read", "Write"].iter().enumerate() {
                if let Some(s) = self.scores[i][j] {
                    text.push_str(&format!(
                        "  {name}: {:.2} MB/s | {:.0} IOPS | {:.2} us\n",
                        s.mbps, s.iops, s.latency_us
                    ));
                }
            }
        }
        text
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let mut done = false;
        if let Some(rx) = &self.receiver {
            while let Ok(event) = rx.try_recv() {
                match event {
                    Event::Status(s) => self.status = s,
                    Event::Score(i, j, s) => self.scores[i][j] = Some(s),
                    Event::Done(result) => {
                        self.status = result
                            .err()
                            .unwrap_or_else(|| "Complete · test file removed".into());
                        done = true;
                    }
                }
            }
        }
        if done {
            self.receiver = None;
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
        let busy = self.receiver.is_some();
        if busy {
            ctx.request_repaint_after(Duration::from_millis(100));
            if !done && self.worker.as_ref().is_some_and(|w| w.is_finished()) {
                // Done may have arrived after this frame drained the channel; process it next frame.
                let worker = self.worker.take().unwrap();
                if worker.join().is_err() {
                    self.receiver = None;
                    self.status = "Benchmark worker failed".into();
                }
            }
        }
        let previous_settings = (
            self.directory.clone(),
            self.size,
            self.repeats,
            self.seconds,
        );
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [76.0, 34.0],
                        egui::Button::new(if busy { "Stop" } else { "All" })
                            .fill(egui::Color32::from_rgb(96, 190, 95)),
                    )
                    .clicked()
                {
                    if busy {
                        self.cancel.store(true, Ordering::Relaxed);
                        self.status = "Stopping…".into();
                    } else {
                        self.start(None, ctx.clone());
                    }
                }
                ui.add_enabled_ui(!busy, |ui| {
                    egui::ComboBox::from_id_salt("runs")
                        .selected_text(format!("{} runs", self.repeats))
                        .width(65.0)
                        .show_ui(ui, |ui| {
                            for n in [1, 3, 5, 9] {
                                ui.selectable_value(&mut self.repeats, n, format!("{n} runs"));
                            }
                        });
                    egui::ComboBox::from_id_salt("size")
                        .selected_text(format!("{} MiB", self.size))
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for n in [64, 256, 1024, 4096, 16384] {
                                ui.selectable_value(&mut self.size, n, format!("{n} MiB"));
                            }
                        });
                    egui::ComboBox::from_id_salt("time")
                        .selected_text(format!("{} s", self.seconds))
                        .width(45.0)
                        .show_ui(ui, |ui| {
                            for n in [1, 3, 5, 10, 30] {
                                ui.selectable_value(&mut self.seconds, n, format!("{n} s"));
                            }
                        });
                });
                egui::ComboBox::from_id_salt("unit")
                    .selected_text(["MB/s", "IOPS", "µs"][self.unit])
                    .width(55.0)
                    .show_ui(ui, |ui| {
                        for (n, label) in ["MB/s", "IOPS", "µs"].iter().enumerate() {
                            ui.selectable_value(&mut self.unit, n, *label);
                        }
                    });
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Folder…"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.directory)
                        .pick_folder()
                {
                    self.directory = path;
                }
                ui.add(egui::Label::new(self.directory.display().to_string()).truncate())
                    .on_hover_text("Select a folder on the drive to test");
            });
            egui::Grid::new("results")
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    ui.label("");
                    ui.strong("Read");
                    ui.strong("Write");
                    ui.end_row();
                    for (i, workload) in WORKLOADS.iter().enumerate() {
                        if ui
                            .add_enabled(
                                !busy,
                                egui::Button::new(workload.name).min_size(egui::vec2(108.0, 49.0)),
                            )
                            .clicked()
                        {
                            self.start(Some(i), ctx.clone());
                        }
                        for j in 0..2 {
                            let text = self.scores[i][j]
                                .map(|s| match self.unit {
                                    0 => format!("{:.2}", s.mbps),
                                    1 => format!("{:.0}", s.iops),
                                    _ => format!("{:.2}", s.latency_us),
                                })
                                .unwrap_or_else(|| "—".into());
                            ui.add_sized(
                                [166.0, 49.0],
                                egui::Label::new(egui::RichText::new(text).size(29.0).strong()),
                            );
                        }
                        ui.end_row();
                    }
                });
            ui.separator();
            ui.add(egui::Label::new(&self.status).truncate())
                .on_hover_text(&self.status);
            ui.horizontal(|ui| {
                if ui.button("Copy results").clicked() {
                    ctx.copy_text(self.report());
                }
                ui.small("Direct I/O · 1 thread · best of measured runs");
            });
        });
        if previous_settings
            != (
                self.directory.clone(),
                self.size,
                self.repeats,
                self.seconds,
            )
        {
            self.scores = [[None; 2]; 4];
            self.status = "Ready".into();
        }
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn main() -> eframe::Result {
    eframe::run_native(
        "Obsidian Disk",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([510.0, 395.0])
                .with_min_inner_size([510.0, 395.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::<App>::default())
        }),
    )
}
