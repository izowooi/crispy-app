mod arabic;
mod bulk;
mod charts;
mod display;
mod i18n;
mod platform;
mod preferences;
mod scanner;
mod translations;

use charts::ChartMode;
use eframe::egui::{self, Color32, RichText};
use i18n::Language;
use scanner::{ScanEvent, ScanHandle, ScanResult};
use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};

const APP_NAME: &str = "DiskZ";
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
// 한글·한자·가나·아랍 문자는 시스템 글꼴에서 가져온다. 파일 이름과 언어 메뉴에 어느 문자든 나올 수 있어
// 전부 넣되, 한자는 언어마다 모양이 달라 지금 언어의 글꼴을 먼저 찾게 한다.
fn install_fonts(ctx: &egui::Context, language: Language) {
    let mut fonts = egui::FontDefinitions::default();
    for (index, path) in platform::font_paths(language).into_iter().enumerate() {
        if let Ok(bytes) = std::fs::read(path) {
            let name = format!("system-{index}");
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);
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
}
impl DiskApp {
    fn idle(language: Language) -> Self {
        Self {
            language,
            raw_path: PathBuf::new(),
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
            disk: None,
            error: None,
            history: Vec::new(),
        }
    }
    fn new(cc: &eframe::CreationContext<'_>, language: Language) -> Self {
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
        install_fonts(&cc.egui_ctx, language);
        Self::idle(language)
    }
    fn start(&mut self, path: PathBuf) {
        if let Some(s) = &self.scan {
            s.cancel.store(true, Ordering::Relaxed);
        }
        self.error = None;
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
    // 샌드박스 앱은 사용자가 이 창에서 직접 고른 폴더만 읽을 수 있다.
    fn choose(&mut self, title: &str, directory: Option<PathBuf>) {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if let Some(directory) = directory {
            dialog = dialog.set_directory(directory);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.start(platform::scan_root(folder));
        }
    }
    fn choose_folder(&mut self) {
        let directory = if self.raw_path.is_dir() {
            Some(self.raw_path.clone())
        } else {
            platform::home_folder()
        };
        self.choose(
            self.language
                .part("Choose a folder to scan", "스캔할 폴더 선택"),
            directory,
        );
    }
    fn choose_disk(&mut self) {
        self.choose(
            self.language.part(
                "Click Open to scan the whole disk",
                "열기를 누르면 디스크 전체를 스캔합니다",
            ),
            Some(PathBuf::from("/")),
        );
    }
    // 디스크 전체를 스캔하면 맨 위 이름이 볼륨 폴더 이름("Data")이 되므로 디스크 이름으로 바꾼다.
    fn label_root(path: &std::path::Path, result: &mut ScanResult) {
        if path == platform::data_volume() {
            result.nodes[0].name = platform::disk_label().into();
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
                    ScanEvent::Preview(mut r) => {
                        self.result_revision = self.result_revision.wrapping_add(1);
                        Self::label_root(&self.raw_path, &mut r);
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
                            Ok(mut r) => {
                                Self::label_root(&self.raw_path, &mut r);
                                self.result = Some(r)
                            }
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
                for (language, _, name) in i18n::LANGUAGES {
                    ui.selectable_value(&mut self.language, language, arabic::visual(name, None));
                }
            });
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
                                if let Ok(d) = scanner::disk_space(&platform::data_volume()) {
                                    ui.small(l.line(format!(
                                        "{} {} · {} {}",
                                        size(d.total),
                                        l.part("total", "전체"),
                                        size(d.available),
                                        l.part("available", "사용 가능")
                                    )));
                                }
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(l.text("Choose disk…", "디스크 선택…")).clicked()
                                    {
                                        self.choose_disk();
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
                                ui.label(RichText::new(l.text("Folder", "폴더")).size(20.0));
                                ui.small(l.text(
                                    "Look inside one folder, such as Downloads or Documents",
                                    "다운로드나 문서처럼 폴더 하나만 봅니다",
                                ));
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(l.text("Choose folder…", "폴더 선택…")).clicked()
                                    {
                                        self.choose_folder();
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.small(l.text(
                        "DiskZ only reads the folder you choose.",
                        "DiskZ 는 직접 고른 폴더만 읽습니다.",
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small(
                            l.text("Or drop a folder here", "폴더를 이 창에 드래그해도 됩니다"),
                        );
                    });
                });
            });
        });
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
            self.start(platform::scan_root(path));
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
                        ui.label(RichText::new(APP_NAME).size(20.0).color(ACCENT));
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
                        ui.label(l.line(format!(
                            "{} · {} {}",
                            size(self.scanned),
                            self.entries,
                            l.part("items checked", "개 확인")
                        )));
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
                    ui.small(l.line(format!("{} {} · {} {}",size(n.logical_bytes),l.part("logical","논리 크기"),size(n.allocated_bytes),l.part("allocated","할당 크기"))));
                    ui.small(l.line(format!("{} {} · {} {}",n.file_count,l.part("files","파일"),n.error_count,l.part("unreadable","접근 오류"))));
                    if self.scan.is_none(){ui.small(l.line(format!("{} {:.2}s",l.part("Scan time","스캔 시간"),r.elapsed.as_secs_f64())));}
                    ui.horizontal(|ui| {if ui.small_button(l.text("Show in Finder","Finder 에서 보기")).clicked(){reveal=Some(scanner::node_path(&r.nodes,id));}
 if ui.small_button(l.text("Copy path","경로 복사")).clicked(){ctx.copy_text(scanner::node_path(&r.nodes,id).display().to_string());}});
                    ui.checkbox(&mut self.allocated,l.text("Use allocated size","할당 크기로 표시"));
                    ui.small(l.text("APFS shared blocks and snapshots can make folder totals differ from disk usage.","APFS 공유 블록과 스냅샷 때문에 폴더 합계와 디스크 사용량은 다를 수 있습니다."));
                });
                if let Some(d)=self.disk {
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new(l.line(format!("{} · {} {}", l.part("Volume", "볼륨"),size(d.available),l.part("available","사용 가능")))).default_open(false).show(ui,|ui| {
                        ui.small(l.line(format!("{} {} / {}",size(d.total.saturating_sub(d.free)),l.part("used","사용 중"),size(d.total))));
                        ui.add(egui::ProgressBar::new(if d.total>0{(d.total-d.free) as f32/d.total as f32}else{0.0}).fill(ACCENT).desired_height(5.0));
                    });
                }
                if r.nodes[0].error_count>0 {
                    ui.add_space(8.0);ui.small(l.text("Some folders couldn't be read. macOS protects them.","일부 폴더를 읽지 못했습니다. macOS 가 보호하는 폴더입니다."));
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
        if self.mode != old_mode
            && let Err(e) = preferences::save_chart(self.mode)
        {
            self.error = Some(e.to_string());
        }
        if self.language != l {
            install_fonts(ctx, self.language);
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut language = Language::detect();
    if let Some(index) = args.iter().position(|a| a == "--lang") {
        language = args
            .get(index + 1)
            .and_then(|s| s.to_str())
            .and_then(Language::from_code)
            .unwrap_or(language);
    }
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([1000.0, 650.0]);
    eframe::run_native(
        APP_NAME,
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(DiskApp::new(cc, language)))),
    )
}

#[cfg(test)]
mod app_tests {
    use super::*;
    #[test]
    fn opening_app_is_idle_and_does_not_scan() {
        let app = DiskApp::idle(Language::English);
        assert!(app.scan.is_none());
        assert!(app.result.is_none());
    }
}
