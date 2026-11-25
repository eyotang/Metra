//! Overlay geometry in physical pixels. Window dimensions remain logical at the API boundary.
#[derive(Clone, Copy, Debug)]
pub struct WorkArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl WorkArea {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

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
pub fn saved_physical(
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

/// Primary area must be first; missing/disconnected saved displays fall back to it.
pub fn saved_work_area(
    areas: &[(WorkArea, f64)],
    position: Option<crate::settings::SavedPosition>,
    version: u8,
) -> Option<(WorkArea, f64)> {
    areas
        .iter()
        .copied()
        .find(|(area, scale)| {
            position.is_some_and(|position| {
                let (x, y) = saved_physical(position, version, *scale);
                area.contains(x, y)
            })
        })
        .or_else(|| areas.first().copied())
}

#[cfg(test)]
mod monitor_tests {
    use super::*;
    use crate::settings::SavedPosition;
    #[test]
    fn restores_secondary_physical_and_legacy_logical_positions() {
        let primary = (
            WorkArea {
                x: 0.,
                y: 0.,
                width: 1920.,
                height: 1080.,
            },
            1.,
        );
        let secondary = (
            WorkArea {
                x: -2560.,
                y: 0.,
                width: 2560.,
                height: 1440.,
            },
            2.,
        );
        let areas = [primary, secondary];
        for (version, x) in [(3, -2000), (2, -1000)] {
            let (area, scale) =
                saved_work_area(&areas, Some(SavedPosition { x, y: 100 }), version).unwrap();
            assert_eq!((area.x, scale), (-2560., 2.));
        }
        let (area, scale) =
            saved_work_area(&areas[..1], Some(SavedPosition { x: -2000, y: 100 }), 3).unwrap();
        assert_eq!((area.x, scale), (0., 1.));
        assert!(saved_work_area(&[], None, 3).is_none());
    }
}

/// Choose the containing or closest work area for a live window. A point in a
/// taskbar strip or just beyond an edge must not jump to an unrelated primary.
pub fn nearby_work_area(areas: &[(WorkArea, f64)], x: f64, y: f64) -> Option<(WorkArea, f64)> {
    areas
        .iter()
        .copied()
        .find(|(area, _)| area.contains(x, y))
        .or_else(|| {
            let distance = |area: WorkArea| {
                let dx = x - x.clamp(area.x, area.x + area.width);
                let dy = y - y.clamp(area.y, area.y + area.height);
                dx * dx + dy * dy
            };
            areas
                .iter()
                .copied()
                .min_by(|(a, _), (b, _)| distance(*a).total_cmp(&distance(*b)))
        })
}

#[cfg(test)]
mod nearby_tests {
    use super::*;
    #[test]
    fn dragging_into_secondary_taskbar_or_past_edge_stays_on_secondary() {
        let areas = [
            (
                WorkArea {
                    x: 0.,
                    y: 40.,
                    width: 1920.,
                    height: 1040.,
                },
                1.,
            ),
            (
                WorkArea {
                    x: 1920.,
                    y: 80.,
                    width: 2560.,
                    height: 1360.,
                },
                2.,
            ),
        ];
        for (x, y) in [(2000., 30.), (4500., 100.), (1920., 100.)] {
            let (area, scale) = nearby_work_area(&areas, x, y).unwrap();
            assert_eq!((area.x, scale), (1920., 2.));
        }
    }
}
