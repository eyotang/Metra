use super::*;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSStatusBarButton, NSView};

pub(super) fn check_tray_template(app: &AppHandle) -> Result<()> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = (|| {
            let marker =
                MainThreadMarker::new().ok_or("Tray inspection requires AppKit's main thread")?;
            let application = NSApplication::sharedApplication(marker);
            let mut templates = Vec::new();
            for window in application.windows() {
                if let Some(view) = window.contentView() {
                    collect_templates(&view, &mut templates);
                }
            }
            require(
                !templates.is_empty(),
                "Could not find the native status-bar button",
            )?;
            require(
                templates.iter().all(|template| *template),
                "Refreshing the status-bar icon cleared its template appearance",
            )
        })();
        let _ = send.send(result);
    })
    .map_err(|error| error.to_string())?;
    receive
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?
}

fn collect_templates(view: &NSView, templates: &mut Vec<bool>) {
    if let Some(button) = view.downcast_ref::<NSStatusBarButton>() {
        if let Some(image) = button.image() {
            templates.push(image.isTemplate());
        }
    }
    for child in view.subviews() {
        collect_templates(&child, templates);
    }
}
