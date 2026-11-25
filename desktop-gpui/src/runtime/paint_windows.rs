//! Composited native pixels, sampled only over a synthetic test backdrop.
use super::*;
use std::{path::Path, time::Duration};
use windows_sys::Win32::Graphics::{
    Dwm::DwmFlush,
    Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
        CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
        SRCCOPY, SelectObject,
    },
};
type Result<T> = std::result::Result<T, String>;
fn require(value: bool, message: &str) -> Result<()> {
    if value { Ok(()) } else { Err(message.into()) }
}
struct Backdrop(u32);
impl Render for Backdrop {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(self.0))
    }
}
pub(super) fn start(cx: &mut App) {
    assert!(std::env::var_os("METRA_GPUI_DEMO").is_some());
    let config = std::env::var_os("METRA_CONFIG_DIR").expect("Isolated configuration");
    let directory = std::path::PathBuf::from(&config).join("native-screens");
    std::fs::create_dir_all(&directory).unwrap();
    let bubble = cx.global::<Runtime>().bubble.unwrap();
    let (work, scale) = bubble
        .update(cx, |_, window, cx| {
            native_window::work_area(window, cx).unwrap()
        })
        .unwrap();
    let view = cx.new(|_| Backdrop(0x28a058));
    let backdrop_view = view.clone();
    let (backdrop, _) = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(
                    px((work.width / scale) as f32),
                    px((work.height / scale) as f32),
                ),
                cx,
            ))),
            titlebar: None,
            focus: false,
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        cx,
        move |window, cx| {
            native_window::move_to(window, work.x as i32, work.y as i32).unwrap();
            window.resize(size(
                px((work.width / scale) as f32),
                px((work.height / scale) as f32),
            ));
            let _ = cx;
            backdrop_view
        },
    )
    .unwrap();
    cx.spawn(async move |cx| {
        for name in ["bubble","capsule","panel"] {
            cx.update(|cx| {
                let dashboard = cx.global::<Runtime>().dashboard.clone();
                dashboard.update(cx, |dashboard,cx| dashboard.save(metra_lib::settings_change::Change::Snap(name=="capsule"),cx));
            });
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let surface = cx.update(|cx| {
                let surface = if name=="panel" { open_panel(Page::Details,cx); cx.global::<Runtime>().panel.unwrap() } else { cx.global::<Runtime>().bubble.unwrap() };
                surface.update(cx, |_,window,_| native_window::move_to(window,(work.x+100.) as i32,(work.y+100.) as i32).unwrap()).unwrap();
                surface
            });
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let green = cx.update(|cx| sample(surface,cx));
            save(&directory.join(format!("{name}-green.png")),&green);
            let corner_pixels = corners(&green);
            assert!(corner_pixels.iter().all(|pixel| pixel.iter().zip([40u8,160,88]).all(|(a,b)| a.abs_diff(b)<=2)), "{name}: opaque corner pixels {corner_pixels:?}");
            assert!(green.2.chunks_exact(4).any(|pixel| pixel[..3].iter().map(|p| *p as u32).sum::<u32>()<150), "Missing painted opaque content");
            cx.update(|cx| view.update(cx, |view,cx| { view.0=0xb43282; cx.notify(); }));
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let purple = cx.update(|cx| sample(surface,cx));
            save(&directory.join(format!("{name}-purple.png")),&purple);
            assert!(corners(&purple).iter().all(|pixel| pixel.iter().zip([180u8,50,130]).all(|(a,b)| a.abs_diff(b)<=2)), "{name}: backdrop did not update through corners");
            let center = ((green.1/2*green.0+green.0/2)*4) as usize;
            assert!(green.2[center..center+3].iter().zip(&purple.2[center..center+3]).all(|(a,b)| a.abs_diff(*b)<=1), "{name}: center is translucent");
            cx.update(|cx| view.update(cx, |view,cx| { view.0=0x28a058; cx.notify(); }));
        }
        cx.update(|cx| {
            backdrop.update(cx, |_,window,_| window.remove_window()).unwrap();
            std::fs::write(Path::new(&config).join("native-smoke.result"), "PASS: composited bubble/capsule/panel corners are transparent and content is opaque").unwrap();
            cx.quit();
        });
    }).detach();
}
fn sample(surface: AnyWindowHandle, cx: &mut App) -> (u32, u32, Vec<u8>) {
    surface
        .update(cx, |_, window, _| {
            let (x, y) = native_window::position(window).unwrap();
            let scale = window.scale_factor();
            let width = (f32::from(window.bounds().size.width) * scale).round() as u32;
            let height = (f32::from(window.bounds().size.height) * scale).round() as u32;
            (width, height, capture(x, y, width, height).unwrap())
        })
        .unwrap()
}
fn corners(surface: &(u32, u32, Vec<u8>)) -> Vec<Vec<u8>> {
    let (width, height, pixels) = surface;
    [
        (1, 1),
        (width - 2, 1),
        (1, height - 2),
        (width - 2, height - 2),
    ]
    .iter()
    .map(|(x, y)| {
        let offset = ((y * width + x) * 4) as usize;
        pixels[offset..offset + 3].to_vec()
    })
    .collect()
}
fn save(path: &Path, surface: &(u32, u32, Vec<u8>)) {
    image::save_buffer(
        path,
        &surface.2,
        surface.0,
        surface.1,
        image::ColorType::Rgba8,
    )
    .unwrap();
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
