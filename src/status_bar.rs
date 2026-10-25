use crate::{
    model::{DashboardSnapshot, Provider},
    settings::{AppSettings, BubblePercentMode},
};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub(crate) struct StatusEntry {
    pub provider: Provider,
    pub label: String,
    pub value: Option<u8>,
    pub stale: bool,
    pub color: String,
}

impl StatusEntry {
    pub fn headline(&self) -> String {
        let value = self.value.map_or_else(|| "—".into(), |n| format!("{n}%"));
        format!(
            "{} {value}{}",
            self.label,
            if self.stale { " ·" } else { "" }
        )
    }
}

pub(crate) fn entries(snapshot: &DashboardSnapshot, settings: &AppSettings) -> Vec<StatusEntry> {
    settings
        .bubble_provider_order
        .iter()
        .filter(|provider| settings.bubble_visible_providers.contains(provider))
        .map(|provider| {
            let (data, label, color) = match provider {
                Provider::Cursor => (
                    &snapshot.cursor,
                    &settings.cursor_bubble_label,
                    &settings.cursor_bubble_color,
                ),
                Provider::Codex => (
                    &snapshot.codex,
                    &settings.codex_bubble_label,
                    &settings.codex_bubble_color,
                ),
                Provider::Claude => (
                    &snapshot.claude,
                    &settings.claude_bubble_label,
                    &settings.claude_bubble_color,
                ),
            };
            let value = data
                .quotas
                .iter()
                .map(|quota| match settings.bubble_percent_mode {
                    BubblePercentMode::Used => quota.used_percent,
                    BubblePercentMode::Remaining => quota.remaining_percent,
                })
                .filter(|value| value.is_finite())
                .reduce(|a, b| match settings.bubble_percent_mode {
                    BubblePercentMode::Used => a.max(b),
                    BubblePercentMode::Remaining => a.min(b),
                })
                .map(|value| value.clamp(0.0, 100.0).round() as u8);
            StatusEntry {
                provider: *provider,
                label: label.clone(),
                value,
                stale: data.stale,
                color: color.clone(),
            }
        })
        .collect()
}

// Windows has no tray title; some Linux panels also show only the icon.
// A small bitmap keeps numeric usage readable without depending on system fonts.
#[cfg(any(target_os = "windows", target_os = "linux", test))]
pub(crate) fn usage_icon(entry: &StatusEntry) -> Vec<u8> {
    let rgb = u32::from_str_radix(entry.color.trim_start_matches('#'), 16).unwrap_or(0x9c83ff);
    let background = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255];
    let luminance = u32::from(background[0]) * 299
        + u32::from(background[1]) * 587
        + u32::from(background[2]) * 114;
    let foreground = if luminance > 140_000 {
        [15, 18, 26, 255]
    } else {
        [255, 255, 255, 255]
    };
    let mut pixels = vec![0; 32 * 32 * 4];
    for y in 1..31 {
        for x in 1..31 {
            if (3..29).contains(&x) || (3..29).contains(&y) {
                let offset = (y * 32 + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&background);
            }
        }
    }
    let mark = match entry.provider {
        Provider::Cursor => 'C',
        Provider::Codex => 'X',
        Provider::Claude => 'A',
    };
    draw_glyph(&mut pixels, mark, 13, 3, foreground);
    let value = entry.value.map_or_else(|| "--".into(), |n| n.to_string());
    let left = (32 - (value.len() * 8 - 2)) / 2;
    for (index, digit) in value.chars().enumerate() {
        draw_glyph(&mut pixels, digit, left + index * 8, 18, foreground);
    }
    if entry.stale {
        for y in 3..7 {
            for x in 25..29 {
                let offset = (y * 32 + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&foreground);
            }
        }
    }
    pixels
}

#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn draw_glyph(pixels: &mut [u8], glyph: char, x: usize, y: usize, color: [u8; 4]) {
    let rows = match glyph {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 2, 2],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        'C' => [7, 4, 4, 4, 7],
        'X' => [5, 5, 2, 5, 5],
        'A' => [2, 5, 7, 5, 5],
        _ => [0, 0, 7, 0, 0],
    };
    for (row, bits) in rows.iter().enumerate() {
        for column in 0..3 {
            if bits & (1 << (2 - column)) != 0 {
                for dy in 0..2 {
                    for dx in 0..2 {
                        let offset = ((y + row * 2 + dy) * 32 + x + column * 2 + dx) * 4;
                        pixels[offset..offset + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProviderSnapshot, ProviderStatus, QuotaWindow};

    fn snapshot() -> DashboardSnapshot {
        let mut cursor =
            ProviderSnapshot::unavailable(Provider::Cursor, ProviderStatus::Available, "");
        cursor.quotas = vec![
            QuotaWindow::from_used_percent("short", 21.4, None, None),
            QuotaWindow::from_used_percent("long", 80.0, None, None),
        ];
        DashboardSnapshot {
            cursor,
            codex: ProviderSnapshot::unavailable(Provider::Codex, ProviderStatus::NotLoggedIn, ""),
            claude: ProviderSnapshot::unavailable(
                Provider::Claude,
                ProviderStatus::NotInstalled,
                "",
            ),
            refreshing: false,
        }
    }

    #[test]
    fn remaining_uses_the_most_constrained_quota() {
        assert_eq!(
            entries(&snapshot(), &AppSettings::default())[0].headline(),
            "C 20%"
        );
    }

    #[test]
    fn used_respects_order_visibility_labels_and_stale_data() {
        let settings = AppSettings {
            bubble_percent_mode: BubblePercentMode::Used,
            bubble_provider_order: vec![Provider::Claude, Provider::Codex, Provider::Cursor],
            bubble_visible_providers: vec![Provider::Cursor, Provider::Codex],
            cursor_bubble_label: "CU".into(),
            ..AppSettings::default()
        };
        let mut snapshot = snapshot();
        snapshot.cursor.stale = true;
        assert_eq!(
            entries(&snapshot, &settings)
                .iter()
                .map(StatusEntry::headline)
                .collect::<Vec<_>>(),
            vec!["X —", "CU 80% ·"]
        );
    }

    #[test]
    fn invalid_quota_values_are_not_reported_as_zero_usage() {
        let mut snapshot = snapshot();
        snapshot.cursor.quotas[0].remaining_percent = f64::NAN;
        snapshot.cursor.quotas[1].remaining_percent = f64::INFINITY;
        assert_eq!(entries(&snapshot, &AppSettings::default())[0].value, None);
    }

    #[test]
    fn icons_render_unknown_zero_and_three_digit_values() {
        let mut entry = entries(&snapshot(), &AppSettings::default()).remove(0);
        for value in [None, Some(0), Some(99), Some(100)] {
            entry.value = value;
            entry.stale = true;
            let pixels = usage_icon(&entry);
            assert_eq!(pixels.len(), 32 * 32 * 4);
            assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] == 255));
        }
    }
}
