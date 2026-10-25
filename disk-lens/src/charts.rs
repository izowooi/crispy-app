use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use std::{cmp::Reverse, f32::consts::TAU, ops::Range};

const TILE_BUDGET: usize = 3000;
const AGGREGATE: usize = usize::MAX;

#[derive(PartialEq, Clone, Copy)]
pub enum ChartMode {
    Sunburst,
    Treemap,
}

fn bytes(nodes: &[crate::scanner::Node], id: usize, allocated: bool) -> u64 {
    if allocated {
        nodes[id].allocated_bytes
    } else {
        nodes[id].logical_bytes
    }
}
fn size(value: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = value as f64;
    let mut i = 0;
    while v >= 1000. && i < 4 {
        v /= 1000.;
        i += 1;
    }
    format!("{v:.1} {}", units[i])
}
fn color(index: usize, depth: usize) -> Color32 {
    let palette = [
        [74, 177, 230],
        [103, 202, 177],
        [155, 139, 235],
        [237, 171, 94],
        [221, 111, 153],
        [102, 153, 232],
        [188, 195, 106],
    ];
    let c = palette[index % palette.len()];
    let factor = (1.0 - depth as f32 * 0.10).max(0.5);
    Color32::from_rgb(
        (c[0] as f32 * factor) as u8,
        (c[1] as f32 * factor) as u8,
        (c[2] as f32 * factor) as u8,
    )
}

// Balanced binary partition keeps large siblings readable without skinny strips.
fn partition_budget(
    rect: Rect,
    weights: &[(usize, u64)],
    budget: usize,
    output: &mut Vec<(usize, Rect)>,
) {
    if weights.is_empty() || budget == 0 {
        return;
    }
    if weights.len() == 1 {
        output.push((weights[0].0, rect));
        return;
    }
    if budget == 1 {
        output.push((AGGREGATE, rect));
        return;
    }
    let total: f64 = weights.iter().map(|x| x.1 as f64).sum();
    if total <= 0. {
        return;
    }
    let mut split = 1;
    let mut left = weights[0].1 as f64;
    while split < weights.len() - 1 && left + weights[split].1 as f64 <= total * 0.5 {
        left += weights[split].1 as f64;
        split += 1;
    }
    let ratio = (left / total) as f32;
    let (a, b) = if rect.width() >= rect.height() {
        let x = rect.left() + rect.width() * ratio;
        (
            Rect::from_min_max(rect.min, Pos2::new(x, rect.bottom())),
            Rect::from_min_max(Pos2::new(x, rect.top()), rect.max),
        )
    } else {
        let y = rect.top() + rect.height() * ratio;
        (
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), y)),
            Rect::from_min_max(Pos2::new(rect.left(), y), rect.max),
        )
    };
    let left_budget =
        ((budget as f64 * split as f64 / weights.len() as f64) as usize).clamp(1, budget - 1);
    partition_budget(a, &weights[..split], left_budget, output);
    partition_budget(b, &weights[split..], budget - left_budget, output);
}
fn children(nodes: &[crate::scanner::Node], id: usize, allocated: bool) -> Vec<(usize, u64)> {
    let mut result: Vec<_> = nodes[id]
        .children
        .iter()
        .map(|&i| (i, bytes(nodes, i, allocated)))
        .filter(|x| x.1 > 0)
        .collect();
    result.sort_unstable_by_key(|a| Reverse(a.1));
    result
}

struct Tile {
    id: usize,
    rect: Rect,
    depth: usize,
    hue: usize,
    aggregate: bool,
}
fn tiles(
    nodes: &[crate::scanner::Node],
    id: usize,
    rect: Rect,
    allocated: bool,
    depth: usize,
    hue: usize,
    out: &mut Vec<Tile>,
) {
    let mut pieces = Vec::new();
    // Small siblings retain their combined area as a parent-linked tile. The
    // global tile budget bounds painting to fewer than 10,000 shapes per frame.
    partition_budget(
        rect,
        &children(nodes, id, allocated),
        TILE_BUDGET - out.len(),
        &mut pieces,
    );
    let first = out.len();
    for (index, (child, r)) in pieces.iter().copied().enumerate() {
        out.push(Tile {
            id: if child == AGGREGATE { id } else { child },
            rect: r,
            depth,
            hue: if depth == 0 { index } else { hue },
            aggregate: child == AGGREGATE,
        });
    }
    for (index, (child, r)) in pieces.into_iter().enumerate() {
        if child != AGGREGATE
            && depth < 2
            && r.width() > 90.
            && r.height() > 65.
            && out.len() < TILE_BUDGET
        {
            let h = out[first + index].hue;
            let inner = Rect::from_min_max(r.min + Vec2::new(4., 25.), r.max - Vec2::splat(4.));
            tiles(nodes, child, inner, allocated, depth + 1, h, out);
        }
    }
}
struct Wedge {
    id: usize,
    start: f32,
    end: f32,
    depth: usize,
    hue: usize,
}
fn wedges(
    nodes: &[crate::scanner::Node],
    id: usize,
    allocated: bool,
    angles: Range<f32>,
    depth: usize,
    hue: usize,
    out: &mut Vec<Wedge>,
) {
    if depth >= 4 {
        return;
    }
    let list = children(nodes, id, allocated);
    let total = bytes(nodes, id, allocated).max(list.iter().map(|x| x.1).sum());
    if total == 0 {
        return;
    }
    let start = angles.start;
    let end = angles.end;
    let mut angle = start;
    for (index, (child, weight)) in list.into_iter().enumerate() {
        let next = angle + (end - start) * (weight as f64 / total as f64) as f32;
        let h = if depth == 0 { index } else { hue };
        if next - angle > 0.002 {
            out.push(Wedge {
                id: child,
                start: angle,
                end: next,
                depth,
                hue: h,
            });
            wedges(nodes, child, allocated, angle..next, depth + 1, h, out);
        }
        angle = next;
    }
}
fn wedge_contains(delta: Vec2, radii: Range<f32>, angles: Range<f32>) -> bool {
    let angle = (delta.angle() + TAU / 4.).rem_euclid(TAU) - TAU / 4.;
    radii.contains(&delta.length()) && angles.contains(&angle)
}

fn polar(center: Pos2, r: f32, a: f32) -> Pos2 {
    center + Vec2::angled(a) * r
}
fn annulus(center: Pos2, inner: f32, outer: f32, start: f32, end: f32) -> egui::Mesh {
    let count = (((end - start) * outer / 5.).ceil() as usize).max(2);
    let mut mesh = egui::Mesh::default();
    for i in 0..=count {
        let a = start + (end - start) * i as f32 / count as f32;
        mesh.colored_vertex(polar(center, inner, a), Color32::WHITE);
        mesh.colored_vertex(polar(center, outer, a), Color32::WHITE);
        if i > 0 {
            let j = (i * 2) as u32;
            mesh.add_triangle(j - 2, j - 1, j);
            mesh.add_triangle(j - 1, j + 1, j);
        }
    }
    mesh
}

pub struct ChartView {
    pub root: usize,
    pub allocated: bool,
    pub mode: ChartMode,
    pub selected: Option<usize>,
    pub language: crate::i18n::Language,
    pub revision: u64,
}

#[derive(Clone, Copy, PartialEq)]
struct CacheKey {
    revision: u64,
    root: usize,
    allocated: bool,
    mode: ChartMode,
    rect: Rect,
    language: crate::i18n::Language,
}
struct CachedTile {
    tile: Tile,
    label: Option<String>,
}
struct CachedWedge {
    wedge: Wedge,
    inner: f32,
    outer: f32,
    normal: Shape,
    highlighted: Shape,
}
#[derive(Default)]
pub struct ChartRenderer {
    key: Option<CacheKey>,
    tiles: Vec<CachedTile>,
    wedges: Vec<CachedWedge>,
    root_size: String,
}
impl ChartRenderer {
    fn prepare(&mut self, nodes: &[crate::scanner::Node], key: CacheKey) -> bool {
        if self.key == Some(key) {
            return false;
        }
        self.key = Some(key);
        self.tiles.clear();
        self.wedges.clear();
        self.root_size = size(bytes(nodes, key.root, key.allocated));
        if key.mode == ChartMode::Treemap {
            let mut list = Vec::new();
            tiles(nodes, key.root, key.rect, key.allocated, 0, 0, &mut list);
            self.tiles = list
                .into_iter()
                .map(|tile| {
                    let r = tile.rect.shrink(1.5);
                    let label = if r.width() > 65. && r.height() > 28. {
                        Some(if tile.aggregate {
                            key.language
                                .text(
                                    "Other items · explore parent folder",
                                    "기타 항목 · 상위 폴더에서 탐색",
                                )
                                .to_owned()
                        } else if tile.depth < 2
                            && r.width() > 90.
                            && r.height() > 65.
                            && !nodes[tile.id].children.is_empty()
                        {
                            format!(
                                "{} · {}",
                                nodes[tile.id].name,
                                size(bytes(nodes, tile.id, key.allocated))
                            )
                        } else if r.height() > 53. {
                            format!(
                                "{}\n{}",
                                nodes[tile.id].name,
                                size(bytes(nodes, tile.id, key.allocated))
                            )
                        } else {
                            nodes[tile.id].name.clone()
                        })
                    } else {
                        None
                    };
                    CachedTile { tile, label }
                })
                .collect();
        } else {
            let center = key.rect.center();
            let radius = key.rect.width().min(key.rect.height()) * 0.48;
            let hole = radius * 0.24;
            let ring = (radius - hole) / 4.;
            let mut list = Vec::new();
            wedges(
                nodes,
                key.root,
                key.allocated,
                -TAU / 4. ..TAU * 0.75,
                0,
                0,
                &mut list,
            );
            self.wedges = list
                .into_iter()
                .map(|wedge| {
                    let inner = hole + wedge.depth as f32 * ring;
                    let outer = inner + ring - 2.;
                    let gap = ((wedge.end - wedge.start) * 0.1).min(0.002);
                    let mut mesh =
                        annulus(center, inner, outer, wedge.start + gap, wedge.end - gap);
                    let mut bright = mesh.clone();
                    let c = color(wedge.hue, wedge.depth);
                    for vertex in &mut mesh.vertices {
                        vertex.color = c;
                    }
                    for vertex in &mut bright.vertices {
                        vertex.color = c.gamma_multiply(1.25);
                    }
                    CachedWedge {
                        wedge,
                        inner,
                        outer,
                        normal: Shape::mesh(mesh),
                        highlighted: Shape::mesh(bright),
                    }
                })
                .collect();
        }
        true
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        nodes: &[crate::scanner::Node],
        view: ChartView,
    ) -> Option<usize> {
        let ChartView {
            root,
            allocated,
            mode,
            selected,
            language,
            revision,
        } = view;
        if root >= nodes.len() {
            return None;
        }
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(
            Vec2::new(available.x.max(100.), available.y.max(200.)),
            Sense::click(),
        );
        let rect = response.rect.shrink(8.);
        let pointer = response.hover_pos();
        self.prepare(
            nodes,
            CacheKey {
                revision,
                root,
                allocated,
                mode,
                rect,
                language,
            },
        );
        let mut hovered = None;
        if mode == ChartMode::Treemap {
            for cached in &self.tiles {
                let tile = &cached.tile;
                let r = tile.rect.shrink(1.5);
                if pointer.is_some_and(|p| r.contains(p)) {
                    hovered = Some(tile.id);
                }
                painter.rect_filled(r, 4., color(tile.hue, tile.depth));
                if let Some(label) = &cached.label {
                    painter.with_clip_rect(r.shrink(5.)).text(
                        r.min + Vec2::splat(7.),
                        Align2::LEFT_TOP,
                        label,
                        FontId::proportional(12.),
                        Color32::from_rgb(15, 26, 40),
                    );
                }
            }
            if let Some(t) = self
                .tiles
                .iter()
                .map(|cached| &cached.tile)
                .rev()
                .find(|t| Some(t.id) == hovered.or(selected))
            {
                painter.rect_stroke(
                    t.rect.shrink(2.),
                    4.,
                    Stroke::new(2_f32, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
            if self.tiles.is_empty() {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    language.text("No child items to display", "표시할 하위 항목이 없습니다"),
                    FontId::proportional(16.),
                    Color32::GRAY,
                );
            }
        } else {
            let center = rect.center();
            let radius = rect.width().min(rect.height()) * 0.48;
            let hole = radius * 0.24;
            for cached in &self.wedges {
                let w = &cached.wedge;
                if pointer.is_some_and(|p| {
                    wedge_contains(p - center, cached.inner..cached.outer, w.start..w.end)
                }) {
                    hovered = Some(w.id);
                }
                painter.add(if Some(w.id) == hovered.or(selected) {
                    cached.highlighted.clone()
                } else {
                    cached.normal.clone()
                });
            }
            if pointer.is_some_and(|p| p.distance(center) < hole - 4.) {
                hovered = nodes[root].parent;
            }
            painter.circle_filled(center, hole - 4., Color32::from_rgb(22, 29, 43));
            painter.text(
                center - Vec2::new(0., 10.),
                Align2::CENTER_CENTER,
                &self.root_size,
                FontId::proportional(22.),
                Color32::WHITE,
            );
            painter.text(
                center + Vec2::new(0., 17.),
                Align2::CENTER_CENTER,
                if nodes[root].parent.is_some() {
                    language.text("Click to go up", "클릭하여 상위로")
                } else {
                    language.text("Selected folder", "선택한 폴더")
                },
                FontId::proportional(11.),
                Color32::GRAY,
            );
        }
        if let Some(id) = hovered {
            response.clone().on_hover_ui_at_pointer(|ui| {
                ui.strong(&nodes[id].name);
                ui.label(size(bytes(nodes, id, allocated)));
                ui.label(crate::display::path(&crate::scanner::node_path(nodes, id)));
                if nodes[id].is_dir {
                    ui.small(if Some(id) == nodes[root].parent {
                        language.text("Click to open parent folder", "클릭하여 상위 폴더로 이동")
                    } else {
                        language.text("Click to explore folder", "클릭하여 폴더 안으로 이동")
                    });
                }
            });
        }
        if response.clicked() { hovered } else { None }
    }
}

#[derive(Default)]
pub struct ItemListCache {
    key: Option<(u64, usize, bool)>,
    filter: String,
    sorted: Vec<usize>,
    visible: Vec<usize>,
}
impl ItemListCache {
    pub fn ids(
        &mut self,
        nodes: &[crate::scanner::Node],
        revision: u64,
        root: usize,
        allocated: bool,
        filter: &str,
    ) -> &[usize] {
        let key = (revision, root, allocated);
        let changed = self.key != Some(key);
        if changed {
            self.key = Some(key);
            self.sorted = nodes[root].children.clone();
            self.sorted
                .sort_unstable_by_key(|&id| Reverse(bytes(nodes, id, allocated)));
        }
        if changed || self.filter != filter {
            self.filter = filter.to_owned();
            let query = crate::display::text(filter).to_lowercase();
            self.visible = self
                .sorted
                .iter()
                .copied()
                .filter(|&id| query.is_empty() || nodes[id].name.to_lowercase().contains(&query))
                .collect();
        }
        &self.visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample_nodes() -> Vec<crate::scanner::Node> {
        let mut nodes = Vec::new();
        for (name, parent, logical, allocated) in [
            ("root", None, 100, 100),
            ("Alpha", Some(0), 30, 70),
            ("Beta", Some(0), 70, 30),
        ] {
            nodes.push(crate::scanner::Node {
                component: name.into(),
                name: name.into(),
                parent,
                children: Vec::new(),
                is_dir: parent.is_none(),
                logical_bytes: logical,
                allocated_bytes: allocated,
                file_count: 1,
                error_count: 0,
            });
        }
        nodes[0].children = vec![1, 2];
        nodes
    }

    #[test]
    fn chart_cache_reuses_geometry_and_rebuilds_on_same_ids_with_new_revision() {
        let mut nodes = sample_nodes();
        let mut renderer = ChartRenderer::default();
        let key = CacheKey {
            revision: 1,
            root: 0,
            allocated: false,
            mode: ChartMode::Treemap,
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 400.)),
            language: crate::i18n::Language::English,
        };
        assert!(renderer.prepare(&nodes, key));
        let first_area = renderer
            .tiles
            .iter()
            .find(|t| t.tile.id == 1)
            .unwrap()
            .tile
            .rect
            .area();
        assert!(
            !renderer.prepare(&nodes, key),
            "hover and selection repaint reuse geometry"
        );
        nodes[1].logical_bytes = 80;
        nodes[2].logical_bytes = 20;
        let revised = CacheKey { revision: 2, ..key };
        assert!(renderer.prepare(&nodes, revised));
        let updated_area = renderer
            .tiles
            .iter()
            .find(|t| t.tile.id == 1)
            .unwrap()
            .tile
            .rect
            .area();
        assert!((updated_area / first_area - 80. / 30.).abs() < 0.001);
        for changed in [
            CacheKey {
                allocated: true,
                ..revised
            },
            CacheKey { root: 1, ..revised },
            CacheKey {
                mode: ChartMode::Sunburst,
                ..revised
            },
            CacheKey {
                rect: key.rect.shrink(20.),
                ..revised
            },
            CacheKey {
                language: crate::i18n::Language::Korean,
                ..revised
            },
        ] {
            assert!(renderer.prepare(&nodes, changed));
            assert!(!renderer.prepare(&nodes, changed));
        }
    }

    #[test]
    fn item_cache_updates_sort_filter_and_scan_revision() {
        let mut nodes = sample_nodes();
        let mut cache = ItemListCache::default();
        assert_eq!(cache.ids(&nodes, 1, 0, false, ""), [2, 1]);
        let pointer = cache.visible.as_ptr();
        assert_eq!(cache.ids(&nodes, 1, 0, false, ""), [2, 1]);
        assert_eq!(cache.visible.as_ptr(), pointer);
        assert_eq!(cache.ids(&nodes, 1, 0, true, ""), [1, 2]);
        assert_eq!(cache.ids(&nodes, 1, 0, true, "ALP"), [1]);
        nodes[2].allocated_bytes = 90;
        assert_eq!(cache.ids(&nodes, 2, 0, true, ""), [2, 1]);
        assert!(cache.ids(&nodes, 2, 1, true, "").is_empty());
    }

    #[test]
    fn partitions_preserve_weighted_area_and_do_not_overlap() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 400.));
        let mut out = Vec::new();
        partition_budget(
            rect,
            &[(0, 50), (1, 30), (2, 15), (3, 5)],
            TILE_BUDGET,
            &mut out,
        );
        assert_eq!(out.len(), 4);
        for (id, r) in &out {
            assert!((r.area() / rect.area() - [0.5, 0.3, 0.15, 0.05][*id]).abs() < 0.0001);
            assert!(rect.contains_rect(*r));
        }
        for i in 0..out.len() {
            for j in i + 1..out.len() {
                assert!(out[i].1.intersect(out[j].1).area() <= 0.01);
            }
        }
    }
    #[test]
    fn bounded_partitions_preserve_all_area() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.));
        let weights: Vec<_> = (0..20000).map(|i| (i, 1)).collect();
        let mut out = Vec::new();
        partition_budget(r, &weights, 100, &mut out);
        assert!(out.len() <= 100);
        assert!(out.iter().any(|x| x.0 == AGGREGATE));
        assert!((out.iter().map(|x| x.1.area()).sum::<f32>() - r.area()).abs() < 1.);
    }
    #[test]
    fn wedge_hit_test_excludes_hole_and_other_angles() {
        assert!(wedge_contains(Vec2::new(60., 0.), 40. ..100., -1. ..1.));
        assert!(!wedge_contains(Vec2::new(20., 0.), 40. ..100., -1. ..1.));
        assert!(!wedge_contains(Vec2::new(-60., 0.), 40. ..100., -1. ..1.));
        assert!(wedge_contains(
            Vec2::new(0., -60.),
            40. ..100.,
            -TAU / 4. ..0.
        ));
    }
    #[test]
    fn annular_mesh_respects_hole_and_has_valid_triangles() {
        let mesh = annulus(Pos2::ZERO, 40., 100., -1., 2.);
        assert!(!mesh.indices.is_empty());
        for v in &mesh.vertices {
            assert!((40. - 0.001..=100.001).contains(&v.pos.to_vec2().length()));
        }
        assert!(
            mesh.indices
                .iter()
                .all(|&i| (i as usize) < mesh.vertices.len())
        );
    }
}
