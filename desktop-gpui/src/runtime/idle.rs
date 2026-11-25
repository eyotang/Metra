//! Native paint delivery after an inactive, compact bubble has been idle.
use super::*;
use std::{sync::atomic::Ordering, time::Duration};

pub(super) fn start(cx: &mut App) {
    assert!(
        std::env::var_os("METRA_GPUI_DEMO").is_some(),
        "Idle smoke requires synthetic data"
    );
    let config = std::env::var_os("METRA_CONFIG_DIR").expect("Isolated test configuration");
    let seconds = std::env::var("METRA_IDLE_SMOKE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(2);
    let dashboard = cx.global::<Runtime>().dashboard.clone();
    dashboard.update(cx, |view, cx| {
        use metra_lib::{settings::BubblePercentMode, settings_change::Change};
        view.save(Change::Snap(true), cx);
        view.save(Change::Percent(BubblePercentMode::Used), cx);
    });
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(1)).await;
        cx.update(|cx| publish(37., cx));
        cx.background_executor().timer(Duration::from_secs(1)).await;
        cx.update(|cx| check(37, cx));
        #[cfg(target_os = "macos")]
        {
            let result = cx.background_executor().spawn(async {
                std::process::Command::new("/usr/bin/open").args(["-a", "Finder"]).status()
            }).await.expect("Activate Finder");
            assert!(result.success());
            cx.background_executor().timer(Duration::from_secs(1)).await;
            cx.update(|_| {
                let marker = objc2::MainThreadMarker::new().unwrap();
                assert!(!objc2_app_kit::NSApplication::sharedApplication(marker).isActive(), "Idle paint acceptance requires an inactive application");
            });
        }
        cx.background_executor().timer(Duration::from_secs(seconds)).await;
        cx.update(|cx| publish(61., cx));
        cx.background_executor().timer(Duration::from_secs(1)).await;
        cx.update(|cx| {
            check(61, cx);
            std::fs::write(std::path::Path::new(&config).join("native-smoke.result"), format!("PASS: compact bubble repainted after {seconds}s idle; 37% -> 61% without focus or expansion")).unwrap();
            cx.quit();
        });
    }).detach();
}

fn publish(used: f64, cx: &mut App) {
    let dashboard = cx.global::<Runtime>().dashboard.clone();
    dashboard.update(cx, |view, cx| {
        let mut snapshot = view.service.snapshot.lock().unwrap();
        let snapshot = &mut *snapshot;
        for data in [
            &mut snapshot.cursor,
            &mut snapshot.codex,
            &mut snapshot.claude,
        ] {
            data.quotas = vec![metra_lib::model::QuotaWindow::from_used_percent(
                "Smoke", used, None, None,
            )];
        }
        cx.notify();
    });
}

fn check(value: u8, cx: &mut App) {
    assert_eq!(
        bubble::LAST_RENDERED_USAGE.load(Ordering::SeqCst),
        value,
        "Inactive capsule did not render new usage"
    );
    assert!(cx.global::<Runtime>().panel.is_none());
    cx.global::<Runtime>()
        .bubble
        .unwrap()
        .update(cx, |_, window, _| {
            assert_eq!(
                window.bounds().size.width,
                px(32.),
                "Usage update expanded the capsule"
            );
            #[cfg(target_os = "windows")]
            check_windows_surface(window);
        })
        .unwrap();
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSStatusBarButton, NSView};
        fn collect(view: &NSView, templates: &mut Vec<bool>) {
            if let Some(button) = view.downcast_ref::<NSStatusBarButton>() {
                if let Some(image) = button.image() {
                    templates.push(image.isTemplate());
                }
            }
            for child in view.subviews() {
                collect(&child, templates);
            }
        }
        let marker = objc2::MainThreadMarker::new().unwrap();
        let mut templates = Vec::new();
        for window in NSApplication::sharedApplication(marker).windows() {
            if let Some(view) = window.contentView() {
                collect(&view, &mut templates);
            }
        }
        assert!(!templates.is_empty(), "Native status button missing");
        assert!(
            templates.iter().all(|value| *value),
            "Native tray lost template appearance"
        );
    }
}

#[cfg(target_os = "windows")]
pub(super) fn check_windows_surface(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::RECT,
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetClientRect, GetWindowLongPtrW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        },
    };
    let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window).unwrap().as_raw()
    else {
        panic!("Expected Win32 surface");
    };
    let width = f32::from(window.bounds().size.width);
    assert!(
        width == 32. || width == 56.,
        "Unexpected bubble logical width"
    );
    let scale = window.scale_factor();
    let mut rect = RECT::default();
    // SAFETY: GPUI owns this HWND and the RECT output is writable.
    unsafe {
        assert_ne!(GetClientRect(handle.hwnd.get() as _, &mut rect), 0);
        let style = GetWindowLongPtrW(handle.hwnd.get() as _, GWL_EXSTYLE) as u32;
        assert_ne!(
            style & WS_EX_TOOLWINDOW,
            0,
            "Bubble must stay out of the taskbar"
        );
        assert_ne!(style & WS_EX_TOPMOST, 0, "Bubble must stay on top");
    }
    assert!(
        ((rect.right - rect.left) as f32 - width * scale).abs() <= 1.,
        "Native client width differs from the first-frame layout"
    );
    assert!(
        ((rect.bottom - rect.top) as f32 - 56. * scale).abs() <= 1.,
        "Native client height differs from the first-frame layout"
    );
}
