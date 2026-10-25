//! Overlay geometry in physical pixels. Window dimensions remain logical at the API boundary.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl WorkArea {
    pub fn clamp(self, x: f64, y: f64, width: f64, height: f64) -> (i32, i32) {
        (
            x.clamp(self.x, self.x + (self.width - width).max(0.))
                .round() as i32,
            y.clamp(self.y, self.y + (self.height - height).max(0.))
                .round() as i32,
        )
    }
    pub fn dock(self, x: f64, y: f64, scale: f64, peek: bool) -> (i32, i32, bool) {
        let right = x + 28. * scale >= self.x + self.width / 2.;
        let width = if peek { 32. } else { 56. } * scale;
        let (x, y) = self.clamp(
            if right {
                self.x + self.width - width
            } else {
                self.x
            },
            y,
            width,
            56. * scale,
        );
        (x, y, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capsule_stays_inside_negative_origin_monitor_at_high_dpi() {
        let work = WorkArea {
            x: -2560.,
            y: -200.,
            width: 2560.,
            height: 1440.,
        };
        assert_eq!(work.dock(-100., 1300., 2., true), (-64, 1128, true));
        assert_eq!(work.dock(-2500., -300., 2., true), (-2560, -200, false));
        assert_eq!(work.dock(-64., 1128., 2., false), (-112, 1128, true));
    }
    #[test]
    fn panels_clamp_to_work_area_even_when_larger_than_screen() {
        let work = WorkArea {
            x: 0.,
            y: 30.,
            width: 800.,
            height: 570.,
        };
        assert_eq!(work.clamp(790., 590., 340., 500.), (460, 100));
        assert_eq!(work.clamp(-10., -10., 900., 900.), (0, 30));
    }
}

/// Position version 2 used logical pixels; older and current formats use physical pixels.
pub(crate) fn saved_physical(
    position: crate::settings::SavedPosition,
    version: u8,
    scale: f64,
) -> (f64, f64) {
    let factor = if version == 2 { scale } else { 1. };
    (position.x as f64 * factor, position.y as f64 * factor)
}

#[cfg(test)]
mod migration_tests {
    use super::saved_physical;
    use crate::settings::SavedPosition;
    #[test]
    fn legacy_logical_position_is_scaled_once_and_physical_positions_are_preserved() {
        let position = SavedPosition { x: -800, y: 140 };
        assert_eq!(saved_physical(position, 2, 1.5), (-1200., 210.));
        for version in [0, 1, 3] {
            assert_eq!(saved_physical(position, version, 1.5), (-800., 140.));
        }
    }
}
