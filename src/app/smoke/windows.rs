use super::*;
use std::{fs::File, path::Path};
use tauri::{PhysicalSize, WindowBuilder};
use windows_sys::Win32::Graphics::{
    Dwm::DwmFlush,
    Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
        CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
        SRCCOPY, SelectObject,
    },
};

pub(super) fn check_transparent_edges(
    app: &AppHandle,
    bubble: &WebviewWindow,
    panel: &WebviewWindow,
) -> Result<()> {
    let (work, _) = work_area(bubble)?;
    let backdrop = WindowBuilder::new(app, "smoke-backdrop")
        .title("Metra transparency acceptance backdrop")
        .decorations(false)
        .shadow(false)
        .focused(false)
        .skip_taskbar(true)
        .background_color(tauri::window::Color(40, 160, 88, 255))
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?;
    backdrop
        .set_position(PhysicalPosition::new(work.x as i32, work.y as i32))
        .map_err(|error| error.to_string())?;
    backdrop
        .set_size(PhysicalSize::new(work.width as u32, work.height as u32))
        .map_err(|error| error.to_string())?;
    backdrop.show().map_err(|error| error.to_string())?;
    settle();
    let directory = app
        .state::<Desktop>()
        .service
        .store
        .path()
        .parent()
        .ok_or("Missing isolated test directory")?
        .join("native-screens");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let desktop = capture(
        work.x as i32,
        work.y as i32,
        work.width as u32,
        work.height as u32,
    )?;
    write_png(
        &directory.join("desktop.png"),
        work.width as u32,
        work.height as u32,
        &desktop,
    )?;
    let background = capture((work.x + work.width / 2.) as i32, work.y as i32 + 40, 1, 1)?;
    require(
        background[1] > background[0].saturating_add(60)
            && background[1] > background[2].saturating_add(30),
        &format!(
            "Native desktop capture did not see the green test backdrop: {:?}",
            &background[..3]
        ),
    )?;
    let mut failures = Vec::new();
    set_peek(app, true, true)?;
    if let Err(error) = check_surface(bubble, &directory, "capsule", &background) {
        failures.push(error);
    }
    set_peek(app, false, false)?;
    if let Err(error) = check_surface(bubble, &directory, "bubble", &background) {
        failures.push(error);
    }
    show_panel(app, "details", false)?;
    if let Err(error) = check_surface(panel, &directory, "panel", &background) {
        failures.push(error);
    }
    hide_panel(app.clone())?;
    backdrop.close().map_err(|error| error.to_string())?;
    require(failures.is_empty(), &failures.join("; "))
}

fn check_surface(
    window: &WebviewWindow,
    directory: &Path,
    name: &str,
    background: &[u8],
) -> Result<()> {
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(8);
    let pixels = loop {
        let pixels = capture(position.x, position.y, size.width, size.height)?;
        let has_accent = pixels.chunks_exact(4).any(|p| {
            p[2] > 110 && p[2] > p[0].saturating_add(25) && p[2] > p[1].saturating_add(25)
        });
        let has_dark = pixels
            .chunks_exact(4)
            .any(|p| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]) < 300);
        if has_accent && has_dark {
            break pixels;
        }
        if Instant::now() >= deadline {
            write_png(
                &directory.join(format!("{name}.png")),
                size.width,
                size.height,
                &pixels,
            )?;
            return Err(format!(
                "{name}: native capture did not see painted application content"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    write_png(
        &directory.join(format!("{name}.png")),
        size.width,
        size.height,
        &pixels,
    )?;
    let corners = [
        (1, 1),
        (size.width - 2, 1),
        (1, size.height - 2),
        (size.width - 2, size.height - 2),
    ]
    .map(|(x, y)| {
        let offset = ((y * size.width + x) * 4) as usize;
        pixels[offset..offset + 3].to_vec()
    });
    std::fs::write(
        directory.join(format!("{name}.json")),
        json!({"background": &background[..3], "corners": corners, "width": size.width, "height": size.height}).to_string(),
    ).map_err(|error| error.to_string())?;
    require(
        corners.iter().all(|corner| {
            corner
                .iter()
                .zip(background)
                .all(|(a, b)| a.abs_diff(*b) <= 2)
        }),
        &format!(
            "{name}: opaque native pixels outside rounded content: {corners:?}; backdrop={:?}",
            &background[..3]
        ),
    )
}

fn capture(x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>> {
    // SAFETY: all GDI objects belong to this short-lived capture. The bitmap is
    // deselected before reading/deletion, and every acquired DC is released.
    unsafe {
        let _ = DwmFlush();
        let screen = GetDC(std::ptr::null_mut());
        require(!screen.is_null(), "Could not acquire desktop capture DC")?;
        let memory = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, width as i32, height as i32);
        if memory.is_null() || bitmap.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            if !memory.is_null() {
                DeleteDC(memory);
            }
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("Could not allocate native capture bitmap".into());
        }
        let previous = SelectObject(memory, bitmap);
        let copied = BitBlt(
            memory,
            0,
            0,
            width as i32,
            height as i32,
            screen,
            x,
            y,
            SRCCOPY | CAPTUREBLT,
        );
        SelectObject(memory, previous);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let lines = GetDIBits(
            memory,
            bitmap,
            0,
            height,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        );
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), screen);
        require(
            copied != 0 && lines == height as i32,
            "Could not read native desktop pixels",
        )?;
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        Ok(pixels)
    }
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|error| error.to_string())?
        .write_image_data(pixels)
        .map_err(|error| error.to_string())
}
