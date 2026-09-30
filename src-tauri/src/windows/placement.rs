//! 便签窗口位置的几何计算（纯函数，不依赖 Tauri）。
//!
//! 便签位置随同步在多台设备间共享，从大屏或多屏设备同步来的坐标在另一台设备上
//! 可能落在屏幕外。这里判断便签是否还能被拖动，并给出移回屏幕内的位置。

/// 逻辑像素下的矩形。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn right(&self) -> f64 {
        self.x + self.width
    }

    fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

/// 标题栏（拖动区域）至少有这么宽露在屏幕内，才算能拖回来。
pub const MIN_VISIBLE_WIDTH: f64 = 48.0;
/// 从便签顶部算起必须完整可见的高度（标题栏的可拖动部分）。
pub const GRIP_HEIGHT: f64 = 32.0;

/// 标题栏顶部这一条是否完整落在某个工作区内（水平方向至少露出 `MIN_VISIBLE_WIDTH`）。
fn grip_reachable(note: &Rect, area: &Rect) -> bool {
    let overlap = note.right().min(area.right()) - note.x.max(area.x);
    let required = MIN_VISIBLE_WIDTH.min(note.width);
    overlap >= required && note.y >= area.y && note.y + GRIP_HEIGHT <= area.bottom()
}

/// 点到矩形的距离平方（点在矩形内为 0）。
fn distance_sq(px: f64, py: f64, area: &Rect) -> f64 {
    let dx = (area.x - px).max(0.0).max(px - area.right());
    let dy = (area.y - py).max(0.0).max(py - area.bottom());
    dx * dx + dy * dy
}

fn clamp_axis(start: f64, length: f64, area_start: f64, area_length: f64) -> f64 {
    if length >= area_length {
        area_start
    } else {
        start.clamp(area_start, area_start + area_length - length)
    }
}

/// 便签的标题栏在任一工作区内可拖动时返回 `None`（保持原位）；
/// 否则返回移入离便签中心最近的工作区后的位置。没有显示器信息时不做处理。
pub fn clamp_to_work_areas(note: Rect, areas: &[Rect]) -> Option<(f64, f64)> {
    let areas: Vec<&Rect> = areas
        .iter()
        .filter(|area| area.width > 0.0 && area.height > 0.0)
        .collect();
    if areas.is_empty() || areas.iter().any(|area| grip_reachable(&note, area)) {
        return None;
    }
    let cx = note.x + note.width / 2.0;
    let cy = note.y + note.height / 2.0;
    let target = areas.iter().min_by(|a, b| {
        distance_sq(cx, cy, a)
            .partial_cmp(&distance_sq(cx, cy, b))
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    Some((
        clamp_axis(note.x, note.width, target.x, target.width),
        clamp_axis(note.y, note.height, target.y, target.height),
    ))
}

/// 贴边吸附：便签边缘离屏幕工作区边缘或其他便签边缘小于这个距离时对齐。
// 吸附只在 Windows 上启用（见 `sticky::snap`），其他平台只在测试中使用。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const SNAP_THRESHOLD: f64 = 12.0;
/// 与其他便签并排或上下相接时保留的间距。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const SNAP_GAP: f64 = 8.0;

/// 两段区间之间的距离（重叠为 0）。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn range_gap(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> f64 {
    (b_start - a_end).max(a_start - b_end).max(0.0)
}

/// 在候选位置中挑离 `current` 最近且不超过阈值的一个。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn nearest_candidate(current: f64, candidates: &[f64]) -> Option<f64> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| (candidate - current).abs() <= SNAP_THRESHOLD)
        .min_by(|a, b| {
            (a - current)
                .abs()
                .partial_cmp(&(b - current).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// 拖动结束后的吸附位置。横、纵两个方向分别处理：
/// - 工作区的四条边（便签贴在屏幕边缘，不含任务栏）；
/// - 与其他便签并排（相隔 `SNAP_GAP`，要求另一方向有重叠）；
/// - 与相邻的便签边缘对齐（左对左、右对右、上对上、下对下，要求另一方向相距不远）。
///
/// 没有候选时返回原位置；对吸附后的位置再次调用结果不变。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn snap_position(note: Rect, areas: &[Rect], peers: &[Rect]) -> (f64, f64) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for area in areas {
        xs.push(area.x);
        xs.push(area.right() - note.width);
        ys.push(area.y);
        ys.push(area.bottom() - note.height);
    }
    let near = SNAP_GAP + SNAP_THRESHOLD;
    for peer in peers {
        let vertical_gap = range_gap(note.y, note.bottom(), peer.y, peer.bottom());
        let horizontal_gap = range_gap(note.x, note.right(), peer.x, peer.right());
        if vertical_gap == 0.0 {
            xs.push(peer.right() + SNAP_GAP);
            xs.push(peer.x - SNAP_GAP - note.width);
        }
        if vertical_gap <= near {
            xs.push(peer.x);
            xs.push(peer.right() - note.width);
        }
        if horizontal_gap == 0.0 {
            ys.push(peer.bottom() + SNAP_GAP);
            ys.push(peer.y - SNAP_GAP - note.height);
        }
        if horizontal_gap <= near {
            ys.push(peer.y);
            ys.push(peer.bottom() - note.height);
        }
    }
    (
        nearest_candidate(note.x, &xs).unwrap_or(note.x),
        nearest_candidate(note.y, &ys).unwrap_or(note.y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1920×1080 主屏，底部任务栏 40 px。
    fn primary() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1040.0)
    }

    fn note_at(x: f64, y: f64) -> Rect {
        Rect::new(x, y, 284.0, 280.0)
    }

    #[test]
    fn keeps_note_inside_screen() {
        assert_eq!(clamp_to_work_areas(note_at(48.0, 76.0), &[primary()]), None);
    }

    #[test]
    fn keeps_note_partly_outside_when_grip_reachable() {
        // 右侧大半露在屏幕外、底部超出屏幕，但标题栏仍可拖动。
        assert_eq!(
            clamp_to_work_areas(note_at(1800.0, 900.0), &[primary()]),
            None
        );
        assert_eq!(
            clamp_to_work_areas(note_at(-200.0, 100.0), &[primary()]),
            None
        );
    }

    #[test]
    fn moves_note_from_missing_monitor_back() {
        // 来自右侧 2560 宽副屏的坐标。
        assert_eq!(
            clamp_to_work_areas(note_at(3000.0, 400.0), &[primary()]),
            Some((1920.0 - 284.0, 400.0))
        );
        // 左侧副屏（负坐标）拔掉后。
        assert_eq!(
            clamp_to_work_areas(note_at(-1500.0, 200.0), &[primary()]),
            Some((0.0, 200.0))
        );
    }

    #[test]
    fn moves_note_when_grip_is_hidden() {
        // 标题栏在屏幕顶部之上。
        assert_eq!(
            clamp_to_work_areas(note_at(100.0, -20.0), &[primary()]),
            Some((100.0, 0.0))
        );
        // 标题栏压在任务栏上（工作区之外）。
        assert_eq!(
            clamp_to_work_areas(note_at(100.0, 1020.0), &[primary()]),
            Some((100.0, 1040.0 - 280.0))
        );
        // 只露出不到 48 px。
        assert_eq!(
            clamp_to_work_areas(note_at(1900.0, 100.0), &[primary()]),
            Some((1920.0 - 284.0, 100.0))
        );
    }

    #[test]
    fn keeps_note_on_secondary_monitor_with_negative_coordinates() {
        let left = Rect::new(-2560.0, -200.0, 2560.0, 1400.0);
        assert_eq!(
            clamp_to_work_areas(note_at(-1500.0, -100.0), &[primary(), left]),
            None
        );
    }

    #[test]
    fn keeps_note_straddling_two_monitors() {
        let right = Rect::new(1920.0, 0.0, 1920.0, 1040.0);
        assert_eq!(
            clamp_to_work_areas(note_at(1800.0, 300.0), &[primary(), right]),
            None
        );
    }

    #[test]
    fn picks_nearest_monitor() {
        let right = Rect::new(1920.0, 0.0, 1920.0, 1040.0);
        // 在右屏正下方，应移到右屏底部而不是主屏。
        assert_eq!(
            clamp_to_work_areas(note_at(3000.0, 1500.0), &[primary(), right]),
            Some((3000.0, 1040.0 - 280.0))
        );
    }

    #[test]
    fn scaled_work_area() {
        // 150% 缩放的 1920×1080 屏幕，逻辑尺寸 1280×693。
        let scaled = Rect::new(0.0, 0.0, 1280.0, 693.0);
        assert_eq!(
            clamp_to_work_areas(note_at(1500.0, 600.0), &[scaled]),
            Some((1280.0 - 284.0, 693.0 - 280.0))
        );
    }

    #[test]
    fn note_larger_than_screen_aligns_to_top_left() {
        let small = Rect::new(0.0, 0.0, 200.0, 150.0);
        assert_eq!(
            clamp_to_work_areas(note_at(500.0, 500.0), &[small]),
            Some((0.0, 0.0))
        );
    }

    #[test]
    fn ignores_missing_monitor_info() {
        assert_eq!(clamp_to_work_areas(note_at(5000.0, 5000.0), &[]), None);
        assert_eq!(
            clamp_to_work_areas(note_at(5000.0, 5000.0), &[Rect::new(0.0, 0.0, 0.0, 0.0)]),
            None
        );
    }

    fn snap(x: f64, y: f64, peers: &[Rect]) -> (f64, f64) {
        snap_position(note_at(x, y), &[primary()], peers)
    }

    #[test]
    fn snaps_to_screen_edges() {
        assert_eq!(snap(9.0, 300.0, &[]), (0.0, 300.0));
        assert_eq!(snap(300.0, 6.0, &[]), (300.0, 0.0));
        // 右下角：底边贴工作区（任务栏之上）。
        assert_eq!(
            snap(1920.0 - 284.0 - 10.0, 1040.0 - 280.0 + 11.0, &[]),
            (1636.0, 760.0)
        );
    }

    #[test]
    fn leaves_note_outside_threshold() {
        assert_eq!(snap(13.0, 300.0, &[]), (13.0, 300.0));
        assert_eq!(snap(500.0, 500.0, &[]), (500.0, 500.0));
    }

    #[test]
    fn snaps_beside_peer_with_gap() {
        let peer = note_at(400.0, 300.0);
        // 放在右侧：左边缘距 peer 右边缘 + 间距 5 px。
        assert_eq!(snap(400.0 + 284.0 + 8.0 + 5.0, 320.0, &[peer]).0, 692.0);
        // 放在左侧。
        assert_eq!(snap(400.0 - 8.0 - 284.0 - 7.0, 320.0, &[peer]).0, 108.0);
    }

    #[test]
    fn stacks_below_peer_and_aligns_left_edge() {
        let peer = note_at(400.0, 100.0);
        assert_eq!(
            snap(406.0, 100.0 + 280.0 + 8.0 + 4.0, &[peer]),
            (400.0, 388.0)
        );
    }

    #[test]
    fn ignores_far_away_peers() {
        // 竖直方向离得很远，不按它的左边缘对齐，也不并排。
        let peer = note_at(400.0, 700.0);
        assert_eq!(snap(405.0, 100.0, &[peer]), (405.0, 100.0));
    }

    #[test]
    fn picks_nearest_candidate() {
        // 左边缘离屏幕 10 px，离 peer 的左边缘 3 px（上下相邻），取 3 px 那个。
        let peer = note_at(13.0, 100.0);
        assert_eq!(snap(10.0, 392.0, &[peer]).0, 13.0);
    }

    #[test]
    fn snapping_is_stable() {
        let peers = [note_at(400.0, 100.0), note_at(700.0, 100.0)];
        for (x, y) in [(9.0, 7.0), (410.0, 395.0), (690.0, 105.0), (1000.0, 1000.0)] {
            let first = snap(x, y, &peers);
            assert_eq!(snap(first.0, first.1, &peers), first);
        }
    }
}
