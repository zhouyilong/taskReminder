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
}
