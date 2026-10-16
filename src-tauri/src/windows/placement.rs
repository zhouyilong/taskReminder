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

/// 便签所在的工作区：重叠面积最大的一个；与所有工作区都不相交时取离便签中心最近的。
pub fn area_of(note: &Rect, areas: &[Rect]) -> Option<Rect> {
    let areas = areas
        .iter()
        .filter(|area| area.width > 0.0 && area.height > 0.0);
    let cx = note.x + note.width / 2.0;
    let cy = note.y + note.height / 2.0;
    areas
        .max_by(|a, b| {
            let by_overlap = overlap_area(note, a)
                .partial_cmp(&overlap_area(note, b))
                .unwrap_or(std::cmp::Ordering::Equal);
            // 重叠相同（如都为 0）时离中心越近越优先。
            by_overlap.then_with(|| {
                distance_sq(cx, cy, b)
                    .partial_cmp(&distance_sq(cx, cy, a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .copied()
}

/// 与工作区同宽或同高的比例下限：达到即视为系统贴靠留下的尺寸。
pub const SNAPPED_RATIO: f64 = 0.95;
/// 贴靠后的尺寸可能比工作区略大（窗口边框），超出这么多以内仍算贴靠。
pub const SNAPPED_OVERFLOW: f64 = 16.0;

/// 便签尺寸是否像被系统贴靠（Aero Snap）贴成了全屏或半屏：宽或高与所在工作区几乎相同。
/// 2.2.2 之前，拖到屏幕边缘会被系统贴靠，这个尺寸会被保存并同步。
/// 比工作区大得多的便签（来自更大屏幕的设备）不算，四分之一屏（半宽半高）无法与手动调大的便签区分，也不算。
pub fn looks_snapped(width: f64, height: f64, area: &Rect) -> bool {
    let fills = |length: f64, area_length: f64| {
        area_length > 0.0
            && length >= area_length * SNAPPED_RATIO
            && length <= area_length + SNAPPED_OVERFLOW
    };
    fills(width, area.width) || fills(height, area.height)
}

/// 贴边吸附：便签边缘离屏幕工作区边缘或其他便签边缘小于这个距离时对齐。
// 吸附只在 Windows 上启用（见 `sticky::snap`），其他平台只在测试中使用。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const SNAP_THRESHOLD: f64 = 16.0;
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

/// 两个矩形相交部分的面积（不相交为 0）。
fn overlap_area(a: &Rect, b: &Rect) -> f64 {
    let width = a.right().min(b.right()) - a.x.max(b.x);
    let height = a.bottom().min(b.bottom()) - a.y.max(b.y);
    if width > 0.0 && height > 0.0 {
        width * height
    } else {
        0.0
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn contains_point(area: &Rect, x: f64, y: f64) -> bool {
    x >= area.x && x < area.right() && y >= area.y && y < area.bottom()
}

/// 拖出屏幕时收回：便签主要所在的工作区（重叠面积最大）的某条边被越过、
/// 且越过的部分不在另一块屏幕上时，把便签移回贴住这条边（任务栏也算越界）。
/// 便签比工作区还大的方向不处理；与所有工作区都不相交时保持原位（打开时的越界校正会处理）。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn keep_inside(note: Rect, areas: &[Rect]) -> (f64, f64) {
    let Some(area) = areas
        .iter()
        .filter(|area| overlap_area(&note, area) > 0.0)
        .max_by(|a, b| {
            overlap_area(&note, a)
                .partial_cmp(&overlap_area(&note, b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        return (note.x, note.y);
    };
    let on_other_area = |x: f64, y: f64| {
        areas
            .iter()
            .any(|other| !std::ptr::eq(other, area) && contains_point(other, x, y))
    };
    let cx = note.x + note.width / 2.0;
    let cy = note.y + note.height / 2.0;
    let mut x = note.x;
    if note.width <= area.width {
        if note.x < area.x && !on_other_area(note.x, cy) {
            x = area.x;
        } else if note.right() > area.right() && !on_other_area(note.right() - 1.0, cy) {
            x = area.right() - note.width;
        }
    }
    let mut y = note.y;
    if note.height <= area.height {
        if note.y < area.y && !on_other_area(cx, note.y) {
            y = area.y;
        } else if note.bottom() > area.bottom() && !on_other_area(cx, note.bottom() - 1.0) {
            y = area.bottom() - note.height;
        }
    }
    (x, y)
}

/// 拖动结束后的吸附位置。横、纵两个方向分别处理：
/// - 工作区的四条边（便签贴在屏幕边缘，不含任务栏）；
/// - 与其他便签并排（相隔 `SNAP_GAP`，要求另一方向有重叠）；
/// - 与相邻的便签边缘对齐（左对左、右对右、上对上、下对下，要求另一方向相距不远）。
///
/// 最后把拖出屏幕的便签收回到边缘（`keep_inside`）。
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
    let x = nearest_candidate(note.x, &xs).unwrap_or(note.x);
    let y = nearest_candidate(note.y, &ys).unwrap_or(note.y);
    keep_inside(Rect::new(x, y, note.width, note.height), areas)
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
    fn area_of_picks_monitor_with_most_overlap() {
        let left = Rect::new(-1280.0, 0.0, 1280.0, 693.0);
        // 大半在左侧副屏上。
        let note = Rect::new(-200.0, 100.0, 284.0, 280.0);
        assert_eq!(area_of(&note, &[primary(), left]), Some(left));
        // 完全在屏幕外时取离中心最近的。
        let lost = Rect::new(2500.0, 100.0, 284.0, 280.0);
        assert_eq!(area_of(&lost, &[left, primary()]), Some(primary()));
        assert_eq!(area_of(&lost, &[]), None);
        assert_eq!(area_of(&lost, &[Rect::new(0.0, 0.0, 0.0, 0.0)]), None);
    }

    #[test]
    fn detects_sizes_left_by_system_snap() {
        let area = primary();
        // 全屏、左右半屏。
        assert!(looks_snapped(1920.0, 1040.0, &area));
        assert!(looks_snapped(960.0, 1040.0, &area));
        // 带一点窗口边框也算。
        assert!(looks_snapped(976.0, 1056.0, &area));
        // 顶边全宽贴靠（Win11 布局）。
        assert!(looks_snapped(1920.0, 520.0, &area));
    }

    #[test]
    fn keeps_ordinary_and_oversized_notes() {
        let area = primary();
        assert!(!looks_snapped(284.0, 280.0, &area));
        // 不足 95%。
        assert!(!looks_snapped(600.0, 980.0, &area));
        // 四分之一屏无法与手动调大的便签区分。
        assert!(!looks_snapped(960.0, 520.0, &area));
        // 比屏幕大得多：来自更大屏幕的设备，不当作贴靠。
        assert!(!looks_snapped(600.0, 1400.0, &area));
        assert!(!looks_snapped(284.0, 280.0, &Rect::new(0.0, 0.0, 0.0, 0.0)));
    }

    #[test]
    fn detects_snap_on_scaled_monitor() {
        // 150% 缩放的 1920×1080 屏幕（逻辑 1280×693）贴成左半屏。
        let scaled = Rect::new(0.0, 0.0, 1280.0, 693.0);
        assert!(looks_snapped(640.0, 693.0, &scaled));
        // 同一尺寸在 100% 的主屏上不算。
        assert!(!looks_snapped(640.0, 693.0, &primary()));
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
        assert_eq!(snap(17.0, 300.0, &[]), (17.0, 300.0));
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
    fn pulls_note_dragged_off_screen_back_to_edge() {
        // 左边、上边拖出屏幕。
        assert_eq!(snap(-120.0, 300.0, &[]), (0.0, 300.0));
        assert_eq!(snap(300.0, -60.0, &[]), (300.0, 0.0));
        // 右边拖出屏幕、底边压到任务栏上。
        assert_eq!(snap(1800.0, 900.0, &[]), (1920.0 - 284.0, 1040.0 - 280.0));
    }

    #[test]
    fn keeps_note_straddling_adjacent_monitor() {
        // 右侧有副屏：越过主屏右边缘的部分在副屏上，不收回。
        let right = Rect::new(1920.0, 0.0, 1920.0, 1040.0);
        assert_eq!(
            snap_position(note_at(1800.0, 300.0), &[primary(), right], &[]),
            (1800.0, 300.0)
        );
        // 副屏比主屏矮：越过副屏底边的部分不在任何屏幕上，收回到副屏底边。
        let short = Rect::new(1920.0, 0.0, 1280.0, 700.0);
        assert_eq!(
            snap_position(note_at(2400.0, 600.0), &[primary(), short], &[]),
            (2400.0, 700.0 - 280.0)
        );
    }

    #[test]
    fn keeps_note_larger_than_screen_in_place() {
        let small = Rect::new(0.0, 0.0, 200.0, 150.0);
        assert_eq!(
            snap_position(note_at(-40.0, -30.0), &[small], &[]),
            (-40.0, -30.0)
        );
    }

    #[test]
    fn leaves_note_outside_all_screens() {
        assert_eq!(snap(3000.0, 300.0, &[]), (3000.0, 300.0));
    }

    #[test]
    fn snapping_is_stable() {
        let peers = [note_at(400.0, 100.0), note_at(700.0, 100.0)];
        for (x, y) in [
            (9.0, 7.0),
            (410.0, 395.0),
            (690.0, 105.0),
            (1000.0, 1000.0),
            (-50.0, 500.0),
        ] {
            let first = snap(x, y, &peers);
            assert_eq!(snap(first.0, first.1, &peers), first);
        }
    }
}
