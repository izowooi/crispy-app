#![cfg_attr(
    all(windows, not(debug_assertions), not(feature = "console")),
    windows_subsystem = "windows"
)]

#[cfg(target_os = "macos")]
mod bulk;
#[cfg(windows)]
#[path = "bulk_windows.rs"]
mod bulk;
mod charts;
mod display;
mod i18n;
mod platform;
mod preferences;
mod scanner;

use charts::ChartMode;
use eframe::egui::{self, Color32, RichText};
use i18n::Language;
use scanner::{ScanEvent, ScanHandle, ScanResult};
use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Duration,
};

const BACKGROUND: Color32 = Color32::from_rgb(23, 28, 38);
const ACCENT: Color32 = Color32::from_rgb(102, 224, 194);
fn size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut n = bytes as f64;
    let mut i = 0;
    while n >= 1000.0 && i < 5 {
        n /= 1000.0;
        i += 1;
    }
    format!("{n:.1} {}", units[i])
}
fn app_bundle() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}
fn open(target: &str) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg(target)
        .spawn()
        .map(|_| ())
}

struct DiskApp {
    language: Language,
    raw_path: PathBuf,
    scan: Option<ScanHandle>,
    result: Option<ScanResult>,
    focus: usize,
    selected: Option<usize>,
    mode: ChartMode,
    chart_renderer: charts::ChartRenderer,
    item_list: charts::ItemListCache,
    result_revision: u64,
    allocated: bool,
    filter: String,
    status: String,
    entries: u64,
    scanned: u64,
    disk: Option<scanner::DiskSpace>,
    error: Option<String>,
    history: Vec<usize>,
    permissions: bool,
}
impl DiskApp {
    fn idle(initial: PathBuf, language: Language) -> Self {
        let disk = scanner::disk_space(&initial).ok();
        Self {
            language,
            raw_path: initial,
            scan: None,
            result: None,
            focus: 0,
            selected: None,
            mode: preferences::load_chart(),
            chart_renderer: charts::ChartRenderer::default(),
            item_list: charts::ItemListCache::default(),
            result_revision: 0,
            allocated: true,
            filter: String::new(),
            status: String::new(),
            entries: 0,
            scanned: 0,
            disk,
            error: None,
            history: Vec::new(),
            permissions: false,
        }
    }
    fn new(cc: &eframe::CreationContext<'_>, initial: PathBuf, language: Language) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.visuals.panel_fill = BACKGROUND;
        style.visuals.override_text_color = Some(Color32::from_rgb(222, 226, 235));
        style.visuals.window_fill = Color32::from_rgb(29, 35, 47);
        style.visuals.selection.bg_fill = Color32::from_rgb(41, 92, 91);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(36, 44, 57);
        cc.egui_ctx.set_style(style);
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read(platform::font_path()) {
            fonts
                .font_data
                .insert("korean".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .push("korean".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        Self::idle(initial, language)
    }
    fn start(&mut self, path: PathBuf) {
        if let Some(s) = &self.scan {
            s.cancel.store(true, Ordering::Relaxed);
        }
        if let Err(e) = preferences::save_last_path(&path) {
            self.error = Some(e.to_string());
        } else {
            self.error = None;
        }
        self.disk = scanner::disk_space(&path).ok();
        self.raw_path = path.clone();
        self.scan = Some(scanner::spawn_scan(path));
        self.result = None;
        self.result_revision = self.result_revision.wrapping_add(1);
        self.focus = 0;
        self.selected = None;
        self.entries = 0;
        self.scanned = 0;
        self.status.clear();
        self.history.clear();
        self.filter.clear();
    }
    fn choose_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title(
            self.language
                .text("Choose a folder to scan", "스캔할 폴더 선택"),
        );
        if self.raw_path.is_dir() {
            dialog = dialog.set_directory(&self.raw_path);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.start(folder);
        }
    }
    fn poll(&mut self) {
        let mut done = false;
        if let Some(s) = &self.scan {
            while let Ok(e) = s.receiver.try_recv() {
                match e {
                    ScanEvent::Progress(p) => {
                        self.entries = p.entries;
                        self.scanned = p.logical_bytes;
                        self.status = display::path(&p.current_path);
                    }
                    ScanEvent::Preview(r) => {
                        self.result_revision = self.result_revision.wrapping_add(1);
                        self.result = Some(r);
                        self.focus = 0;
                        self.selected = None;
                        self.history.clear();
                    }
                    ScanEvent::Finished(r) => {
                        self.result_revision = self.result_revision.wrapping_add(1);
                        done = true;
                        self.focus = 0;
                        self.selected = None;
                        self.history.clear();
                        self.status.clear();
                        match r {
                            Ok(r) => self.result = Some(r),
                            Err(e) => self.error = Some(e),
                        }
                    }
                }
            }
        }
        if done {
            self.scan = None;
            self.disk = scanner::disk_space(&self.raw_path).ok();
        }
    }
    fn navigate(&mut self, id: usize) {
        if self.scan.is_some() {
            return;
        }
        if self.result.as_ref().is_some_and(|r| r.nodes[id].is_dir) && id != self.focus {
            self.history.push(self.focus);
            self.focus = id;
            self.selected = None;
            self.filter.clear();
        } else {
            self.selected = Some(id);
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        let l = self.language;
        ui.menu_button("···", |ui| {
            if ui.button(l.text("Choose folder…", "폴더 선택…")).clicked() {
                ui.close();
                self.choose_folder();
            }
            ui.separator();
            ui.menu_button(l.text("View", "보기"), |ui| {
                ui.selectable_value(
                    &mut self.mode,
                    ChartMode::Treemap,
                    l.text("Treemap", "트리맵"),
                );
                ui.selectable_value(
                    &mut self.mode,
                    ChartMode::Sunburst,
                    l.text("Sunburst", "방사형"),
                );
            });
            ui.menu_button(l.text("Language", "언어"), |ui| {
                ui.selectable_value(&mut self.language, Language::English, "English");
                ui.selectable_value(&mut self.language, Language::Korean, "한국어");
            });
            if cfg!(target_os = "macos")
                && ui
                    .button(l.text("Full Disk Access…", "전체 디스크 접근 권한…"))
                    .clicked()
            {
                self.permissions = true;
                ui.close();
            }
        })
        .response
        .on_hover_text(l.text("Settings", "설정"));
    }
    fn welcome(&mut self, ui: &mut egui::Ui) {
        let l = self.language;
        ui.add_space(65.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(l.text("A clearer view of your storage.", "저장 공간을 한눈에."))
                    .size(30.0),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new(l.text(
                    "Choose a disk or folder to begin.",
                    "디스크나 폴더를 선택해 시작하세요.",
                ))
                .color(Color32::GRAY),
            );
            ui.add_space(40.0);
        });
        let width = ui.available_width().min(720.0);
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            ui.vertical(|ui| {
                ui.set_width(width);
                egui::Frame::new()
                    .fill(Color32::from_rgb(31, 38, 50))
                    .corner_radius(14)
                    .inner_margin(22.0)
                    .show(ui, |ui| {
                        ui.set_width(width - 44.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(platform::disk_label()).size(21.0));
                                if let Ok(d) = scanner::disk_space(&platform::system_disk()) {
                                    ui.small(format!(
                                        "{} {} · {} {}",
                                        size(d.total),
                                        l.text("total", "전체"),
                                        size(d.available),
                                        l.text("available", "사용 가능")
                                    ));
                                }
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(l.text("Scan disk", "디스크 스캔")).clicked()
                                    {
                                        self.start(platform::system_disk());
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(12.0);
                egui::Frame::new()
                    .fill(Color32::from_rgb(31, 38, 50))
                    .corner_radius(14)
                    .inner_margin(22.0)
                    .show(ui, |ui| {
                        ui.set_width(width - 44.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(display::text(
                                        &self
                                            .raw_path
                                            .file_name()
                                            .unwrap_or(self.raw_path.as_os_str())
                                            .to_string_lossy(),
                                    ))
                                    .size(20.0),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(display::path(&self.raw_path))
                                            .small()
                                            .color(Color32::GRAY),
                                    )
                                    .truncate()
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(l.text("Scan folder", "폴더 스캔")).clicked() {
                                        self.start(self.raw_path.clone());
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(l.text("Choose another folder…", "다른 폴더 선택…"))
                        .clicked()
                    {
                        self.choose_folder();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small(
                            l.text("Or drop a folder here", "폴더를 이 창에 드래그해도 됩니다"),
                        );
                    });
                });
            });
        });
    }
    fn permissions_window(&mut self, ctx: &egui::Context) {
        if !self.permissions {
            return;
        }
        let l = self.language;
        let mut visible = true;
        egui::Window::new(l.text("Full Disk Access","전체 디스크 접근 권한")).open(&mut visible).collapsible(false).resizable(false).default_width(490.0).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui| {
            ui.label(l.text("To include protected folders, add Disk Lens in macOS settings. You can also scan the files already available.","보호된 폴더까지 확인하려면 macOS 설정에 디스크 렌즈를 추가하세요. 현재 접근 가능한 파일만 스캔할 수도 있습니다."));
            ui.add_space(12.0);
            ui.label(l.text("1. Open Privacy & Security → Full Disk Access.","1. 개인정보 보호 및 보안 → 전체 디스크 접근 권한을 여세요."));
            ui.label(l.text("2. Click +, choose Disk Lens.app, then turn it on.","2. +를 눌러 Disk Lens.app을 추가하고 스위치를 켜세요."));
            ui.label(l.text("3. Restart Disk Lens, then choose Scan.","3. 디스크 렌즈를 다시 실행한 뒤 스캔을 누르세요."));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(l.text("Open settings","설정 열기")).clicked() && let Err(e)=open("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles") {self.error=Some(e.to_string());}
                if ui.button(l.text("Locate this app","앱 위치 보기")).clicked() {
                    if let Some(bundle)=app_bundle(){let _=std::process::Command::new("open").arg("-R").arg(bundle).spawn();}
                    else{self.error=Some(l.text("Launch the .app bundle to grant access.","권한을 설정하려면 .app 번들로 실행하세요.").into());}
                }
            });
            if let Some(bundle)=app_bundle() {
                ui.add(egui::Label::new(RichText::new(display::path(&bundle)).small().color(Color32::GRAY)).wrap());
                if ui.small_button(l.text("Copy app path","앱 경로 복사")).clicked(){ctx.copy_text(bundle.display().to_string());}
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button(l.text("Restart app","앱 다시 실행")).clicked() && let Some(bundle)=app_bundle() {
                    match std::process::Command::new("open").arg("-n").arg(bundle).spawn() {Ok(_)=>ctx.send_viewport_cmd(egui::ViewportCommand::Close),Err(e)=>self.error=Some(e.to_string())}
                }
            });
        });
        if !visible {
            self.permissions = false;
        }
    }
}
impl Drop for DiskApp {
    fn drop(&mut self) {
        if let Some(s) = &self.scan {
            s.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl eframe::App for DiskApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        let l = self.language;
        let old_mode = self.mode;
        if self.scan.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone())) {
            self.start(path);
        }
        egui::TopBottomPanel::top("navigation")
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(18.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.result.is_some() || self.scan.is_some() {
                        if ui
                            .add_enabled(
                                self.scan.is_none(),
                                egui::Button::new(l.text("Locations", "위치")),
                            )
                            .clicked()
                        {
                            self.result = None;
                            self.selected = None;
                            self.history.clear();
                        }
                        if ui
                            .add_enabled(!self.history.is_empty(), egui::Button::new("←"))
                            .clicked()
                            && let Some(id) = self.history.pop()
                        {
                            self.focus = id;
                            self.selected = None;
                        }
                        if let Some(r) = &self.result {
                            let mut ids = vec![self.focus];
                            let mut p = r.nodes[self.focus].parent;
                            while let Some(id) = p {
                                ids.push(id);
                                p = r.nodes[id].parent;
                            }
                            let mut target = None;
                            for id in ids.into_iter().rev().take(5) {
                                if ui.button(&r.nodes[id].name).clicked() {
                                    target = Some(id);
                                }
                            }
                            if let Some(id) = target {
                                self.navigate(id);
                            }
                        } else {
                            ui.label(display::text(
                                &self
                                    .raw_path
                                    .file_name()
                                    .unwrap_or(self.raw_path.as_os_str())
                                    .to_string_lossy(),
                            ));
                        }
                    } else {
                        ui.label(
                            RichText::new(l.text("Disk Lens", "디스크 렌즈"))
                                .size(20.0)
                                .color(ACCENT),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.settings(ui);
                        if let Some(s) = &self.scan {
                            if ui.button(l.text("Stop", "중지")).clicked() {
                                s.cancel.store(true, Ordering::Relaxed);
                            }
                        } else if self.result.is_some()
                            && ui.button(l.text("Rescan", "다시 스캔")).clicked()
                        {
                            self.start(self.raw_path.clone());
                        }
                    });
                });
            });
        if self.scan.is_some()
            || self.error.is_some()
            || self.result.as_ref().is_some_and(|r| r.cancelled)
        {
            egui::TopBottomPanel::bottom("scan_status").show(ctx, |ui| {
                if self.scan.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!(
                            "{} · {} {}",
                            size(self.scanned),
                            self.entries,
                            l.text("items checked", "개 확인")
                        ));
                        ui.add(
                            egui::Label::new(
                                RichText::new(&self.status).small().color(Color32::GRAY),
                            )
                            .truncate(),
                        );
                    });
                } else if self.result.as_ref().is_some_and(|r| r.cancelled) {
                    ui.small(l.text("Scan stopped · Partial results", "스캔 중지됨 · 부분 결과"));
                }
                if let Some(e) = &self.error {
                    ui.colored_label(Color32::from_rgb(255, 170, 140), e);
                }
            });
        }
        let mut nav = None;
        let mut reveal = None;
        if let Some(r) = &self.result {
            egui::SidePanel::right("summary").default_width(340.0).min_width(290.0).frame(egui::Frame::new().fill(BACKGROUND).inner_margin(24.0)).show(ctx,|ui| {
                let id=self.selected.unwrap_or(self.focus);let n=&r.nodes[id];
                ui.label(RichText::new(&n.name).size(22.0));
                ui.label(RichText::new(size(if self.allocated{n.allocated_bytes}else{n.logical_bytes})).size(34.0).color(ACCENT));
                ui.add_space(8.0);
                egui::CollapsingHeader::new(l.text("Details","상세 정보")).default_open(false).show(ui,|ui| {
                    ui.small(display::path(&scanner::node_path(&r.nodes,id)));
                    ui.small(format!("{} {} · {} {}",size(n.logical_bytes),l.text("logical","논리 크기"),size(n.allocated_bytes),l.text("allocated","할당 크기")));
                    ui.small(format!("{} {} · {} {}",n.file_count,l.text("files","파일"),n.error_count,l.text("unreadable","접근 오류")));
                    if self.scan.is_none(){ui.small(format!("{} {:.2}s",l.text("Scan time","스캔 시간"),r.elapsed.as_secs_f64()));}
                    ui.horizontal(|ui| {if ui.small_button(l.text("Show in file manager","파일 탐색기에서 보기")).clicked(){reveal=Some(scanner::node_path(&r.nodes,id));}
 if ui.small_button(l.text("Copy path","경로 복사")).clicked(){ctx.copy_text(scanner::node_path(&r.nodes,id).display().to_string());}});
                    ui.checkbox(&mut self.allocated,l.text("Use allocated size","할당 크기로 표시"));
                    if cfg!(target_os = "macos") { ui.small(l.text("APFS shared blocks and snapshots can make folder totals differ from disk usage.","APFS 공유 블록과 스냅샷 때문에 폴더 합계와 디스크 사용량은 다를 수 있습니다.")); }
                    else { ui.small(l.text("Protected files, reparse points and filesystem metadata can make totals differ from disk usage.","보호된 파일, 재분석 지점, 파일 시스템 메타데이터 때문에 합계와 디스크 사용량은 다를 수 있습니다.")); }
                });
                if let Some(d)=self.disk {
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new(format!("{} · {} {}", l.text("Volume", "볼륨"),size(d.available),l.text("available","사용 가능"))).default_open(false).show(ui,|ui| {
                        ui.small(format!("{} {} / {}",size(d.total.saturating_sub(d.free)),l.text("used","사용 중"),size(d.total)));
                        ui.add(egui::ProgressBar::new(if d.total>0{(d.total-d.free) as f32/d.total as f32}else{0.0}).fill(ACCENT).desired_height(5.0));
                    });
                }
                if r.nodes[0].error_count>0 {
                    ui.add_space(8.0);ui.small(l.text("Some folders couldn't be read.","일부 폴더를 읽지 못했습니다."));
                    if cfg!(target_os = "macos") && ui.small_button(l.text("Allow disk access…","디스크 접근 허용…")).clicked(){self.permissions=true;}
                }
                ui.add_space(20.0);
                ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text(l.text("Find an item…","항목 찾기…")).desired_width(f32::INFINITY));
                ui.add_space(8.0);
                let ids=self.item_list.ids(&r.nodes,self.result_revision,self.focus,self.allocated,&self.filter);
                egui::ScrollArea::vertical().show_rows(ui,34.0,ids.len(),|ui,range|{for i in range {
                    let id=ids[i];let n=&r.nodes[id];let row=ui.allocate_response(egui::vec2(ui.available_width(),32.0),egui::Sense::click());
                    if row.hovered() || self.selected==Some(id){ui.painter().rect_filled(row.rect,5.0,Color32::from_rgb(39,49,62));}
                    ui.painter().circle_filled(egui::pos2(row.rect.left()+6.0,row.rect.center().y),3.0,if n.is_dir{ACCENT}else{Color32::GRAY});
                    let bytes=size(if self.allocated{n.allocated_bytes}else{n.logical_bytes});
                    ui.painter().text(egui::pos2(row.rect.right(),row.rect.center().y),egui::Align2::RIGHT_CENTER,bytes,egui::FontId::proportional(14.0),Color32::from_gray(205));
                    let clip=egui::Rect::from_min_max(row.rect.min+egui::vec2(18.0,0.0),row.rect.max-egui::vec2(84.0,0.0));
                    ui.painter().with_clip_rect(clip).text(egui::pos2(clip.left(),row.rect.center().y),egui::Align2::LEFT_CENTER,&n.name,egui::FontId::proportional(14.0),Color32::from_gray(225));
                    if row.clicked(){self.selected=Some(id);}
 if row.double_clicked(){nav=Some(id);}row.on_hover_text(display::path(&scanner::node_path(&r.nodes,id)));
                }});
            });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(20.0))
            .show(ctx, |ui| {
                if let Some(r) = &self.result {
                    if let Some(id) = self.chart_renderer.show(
                        ui,
                        &r.nodes,
                        charts::ChartView {
                            root: self.focus,
                            allocated: self.allocated,
                            mode: self.mode,
                            selected: self.selected,
                            language: l,
                            revision: self.result_revision,
                        },
                    ) {
                        nav = Some(id);
                    }
                } else if self.scan.is_some() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(160.0);
                        ui.spinner();
                        ui.label(l.text(
                            "Looking through your storage…",
                            "저장 공간을 살펴보고 있습니다…",
                        ));
                    });
                } else {
                    self.welcome(ui);
                }
            });
        if let Some(id) = nav {
            self.navigate(id);
        }
        if let Some(path) = reveal
            && let Err(e) = platform::reveal(&path)
        {
            self.error = Some(e.to_string());
        }
        self.permissions_window(ctx);
        if self.mode != old_mode
            && let Err(e) = preferences::save_chart(self.mode)
        {
            self.error = Some(e.to_string());
        }
        if self.language != l {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                self.language.text("Disk Lens", "디스크 렌즈").into(),
            ));
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--scan") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage: disk-lens --scan PATH");
            std::process::exit(2)
        };
        match scanner::scan_path(
            Path::new(path),
            &std::sync::atomic::AtomicBool::new(false),
            None,
        ) {
            Ok(r) => {
                let n = &r.nodes[0];
                println!(
                    "{}\nlogical={} allocated={} files={} errors={} seconds={:.3}",
                    r.root.display(),
                    n.logical_bytes,
                    n.allocated_bytes,
                    n.file_count,
                    n.error_count,
                    r.elapsed.as_secs_f64()
                );
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1)
            }
        }
        return Ok(());
    }
    let mut gui_args = args.clone();
    let mut language = Language::detect();
    if let Some(index) = gui_args.iter().position(|a| a == "--lang") {
        language = match gui_args.get(index + 1).and_then(|s| s.to_str()) {
            Some("ko") => Language::Korean,
            Some("en") => Language::English,
            _ => {
                eprintln!("Usage: disk-lens [PATH] [--lang en|ko]");
                std::process::exit(2)
            }
        };
        gui_args.drain(index..=index + 1);
    }
    let initial = gui_args
        .first()
        .map(PathBuf::from)
        .or_else(preferences::load_last_path)
        .unwrap_or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/Users"))
        });
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([1000.0, 650.0]);
    #[cfg(windows)]
    let viewport = viewport.with_icon(
        eframe::icon_data::from_png_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/disk-lens.ico.png"
        )))
        .expect("Generated app icon is valid PNG"),
    );
    eframe::run_native(
        language.text("Disk Lens", "디스크 렌즈"),
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(DiskApp::new(cc, initial, language)))),
    )
}

#[cfg(test)]
mod app_tests {
    use super::*;
    #[test]
    fn opening_app_is_idle_and_does_not_scan() {
        let app = DiskApp::idle(std::env::temp_dir(), Language::English);
        assert!(app.scan.is_none());
        assert!(app.result.is_none());
        assert!(!app.permissions);
    }
}
