use super::*;
use std::sync::{atomic::AtomicU64, Mutex};
use windows_sys::Win32::Foundation::POINT;

struct PetWindowInputState {
    interaction_signal: Arc<AtomicU64>,
    drag_state: Mutex<PetWindowDragState>,
}

#[derive(Default)]
struct PetWindowDragState {
    mouse_down_at: Option<POINT>,
    drag_started: bool,
}

pub fn prepare_window(window: &Window, interaction_signal: &Arc<AtomicU64>) -> LocalResult<()> {
    set_native_window_style(window, interaction_signal)
}

pub fn paint_window(
    window: &Window,
    frame_path: &Path,
    bubble_text: Option<&str>,
    bubble_theme: PetBubbleTheme,
) -> LocalResult<()> {
    let image = image::open(frame_path)
        .map_err(|err| format!("读取宠物图片失败 {}: {err}", frame_path.display()))?
        .to_rgba8();
    let (source_width, source_height) = image.dimensions();
    let scale = window.scale_factor().map_err(|err| err.to_string())?;
    let window_width = (PET_WINDOW_WIDTH * scale).round().max(1.0) as u32;
    let window_height = (PET_WINDOW_HEIGHT * scale).round().max(1.0) as u32;
    let sprite_width = (PET_SPRITE_WIDTH as f64 * scale).round().max(1.0) as u32;
    let sprite_height = (PET_SPRITE_HEIGHT as f64 * scale).round().max(1.0) as u32;
    let mut buffer = vec![0u8; (window_width as usize) * (window_height as usize) * 4];
    let target_x = window_width.saturating_sub(sprite_width) / 2;
    let target_y = window_height.saturating_sub(sprite_height + (6.0 * scale).round() as u32);

    for y in 0..sprite_height {
        for x in 0..sprite_width {
            let source_x = x * source_width / sprite_width;
            let source_y = y * source_height / sprite_height;
            let pixel = image.get_pixel(source_x, source_y).0;
            let destination_x = target_x + x;
            let destination_y = target_y + y;
            let destination_index = ((destination_y * window_width + destination_x) * 4) as usize;
            let alpha = u16::from(pixel[3]);

            buffer[destination_index] = ((u16::from(pixel[2]) * alpha + 127) / 255) as u8;
            buffer[destination_index + 1] = ((u16::from(pixel[1]) * alpha + 127) / 255) as u8;
            buffer[destination_index + 2] = ((u16::from(pixel[0]) * alpha + 127) / 255) as u8;
            buffer[destination_index + 3] = pixel[3];
        }
    }

    if bubble_text.is_some_and(|value| !value.trim().is_empty()) {
        draw_bubble(&mut buffer, window_width, scale, bubble_theme);
    }

    let _ = window.set_size(tauri::LogicalSize::new(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT));
    paint_layered_window(
        window,
        &buffer,
        window_width as i32,
        window_height as i32,
        scale,
        bubble_text,
        bubble_theme,
    )
}

fn set_native_window_style(
    window: &Window,
    interaction_signal: &Arc<AtomicU64>,
) -> LocalResult<()> {
    let hwnd = window.hwnd().map_err(|err| err.to_string())?;
    let input_state = Box::new(PetWindowInputState {
        interaction_signal: interaction_signal.clone(),
        drag_state: Mutex::new(PetWindowDragState::default()),
    });
    unsafe {
        use windows_sys::Win32::UI::{
            Shell::SetWindowSubclass,
            WindowsAndMessaging::{
                GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE,
                WS_EX_TOOLWINDOW,
            },
        };
        let hwnd = hwnd.0 as *mut std::ffi::c_void;
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        #[cfg(target_pointer_width = "64")]
        let next_style =
            style | WS_EX_LAYERED as isize | WS_EX_TOOLWINDOW as isize | WS_EX_NOACTIVATE as isize;
        #[cfg(target_pointer_width = "32")]
        let next_style =
            style | WS_EX_LAYERED as i32 | WS_EX_TOOLWINDOW as i32 | WS_EX_NOACTIVATE as i32;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_style);
        SetWindowSubclass(
            hwnd,
            Some(pet_window_subclass_proc),
            1,
            Box::into_raw(input_state) as usize,
        );
    }
    Ok(())
}

unsafe extern "system" fn pet_window_subclass_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _subclass_id: usize,
    ref_data: usize,
) -> isize {
    use windows_sys::Win32::UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass},
        WindowsAndMessaging::{
            HTCLIENT, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST,
        },
    };

    if msg == WM_NCHITTEST {
        return HTCLIENT as isize;
    }

    let input_state = (ref_data != 0).then(|| &*(ref_data as *const PetWindowInputState));
    match msg {
        WM_LBUTTONDOWN => {
            if let Some(input_state) = input_state {
                record_mouse_down(input_state);
            }
        }
        WM_MOUSEMOVE => {
            if let Some(input_state) = input_state {
                if should_begin_drag(input_state) {
                    begin_window_drag(hwnd);
                }
            }
        }
        WM_LBUTTONUP => {
            if let Some(input_state) = input_state {
                finish_mouse_interaction(input_state);
            }
        }
        WM_DESTROY => {
            if ref_data != 0 {
                RemoveWindowSubclass(hwnd, Some(pet_window_subclass_proc), _subclass_id);
                drop(Box::from_raw(ref_data as *mut PetWindowInputState));
            }
        }
        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe fn record_mouse_down(input_state: &PetWindowInputState) {
    if let Ok(mut guard) = input_state.drag_state.lock() {
        guard.mouse_down_at = cursor_position();
        guard.drag_started = false;
    }
}

unsafe fn should_begin_drag(input_state: &PetWindowInputState) -> bool {
    let Some(current) = cursor_position() else {
        return false;
    };

    let Ok(mut guard) = input_state.drag_state.lock() else {
        return false;
    };

    if guard.drag_started {
        return false;
    }

    let Some(start) = guard.mouse_down_at else {
        return false;
    };

    let delta_x = (current.x - start.x).abs();
    let delta_y = (current.y - start.y).abs();
    if delta_x < 4 && delta_y < 4 {
        return false;
    }

    guard.drag_started = true;
    true
}

fn finish_mouse_interaction(input_state: &PetWindowInputState) {
    let should_interact = input_state
        .drag_state
        .lock()
        .map(|mut guard| {
            let should_interact = guard.mouse_down_at.is_some() && !guard.drag_started;
            guard.mouse_down_at = None;
            guard.drag_started = false;
            should_interact
        })
        .unwrap_or(false);

    if should_interact {
        input_state
            .interaction_signal
            .fetch_add(1, Ordering::Relaxed);
    }
}

unsafe fn cursor_position() -> Option<POINT> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut cursor = POINT { x: 0, y: 0 };
    if GetCursorPos(&mut cursor) == 0 {
        return None;
    }
    Some(cursor)
}

unsafe fn begin_window_drag(hwnd: *mut std::ffi::c_void) {
    use windows_sys::Win32::{
        Foundation::POINTS,
        UI::{
            Input::KeyboardAndMouse::ReleaseCapture,
            WindowsAndMessaging::{PostMessageW, HTCAPTION, WM_NCLBUTTONDOWN},
        },
    };

    let Some(cursor) = cursor_position() else {
        return;
    };

    let points = POINTS {
        x: cursor.x as i16,
        y: cursor.y as i16,
    };

    ReleaseCapture();
    PostMessageW(
        hwnd,
        WM_NCLBUTTONDOWN,
        HTCAPTION as usize,
        &points as *const POINTS as isize,
    );
}

fn draw_bubble(buffer: &mut [u8], width: u32, scale: f64, bubble_theme: PetBubbleTheme) {
    let bubble_x = (20.0 * scale).round() as u32;
    let bubble_y = (8.0 * scale).round() as u32;
    let bubble_width = width.saturating_sub((40.0 * scale).round() as u32).max(1);
    let bubble_height = (64.0 * scale).round().max(1.0) as u32;
    let shadow_y = bubble_y + (3.0 * scale).round() as u32;
    fill_rounded_rect(
        buffer,
        width,
        bubble_x + (2.0 * scale).round() as u32,
        shadow_y,
        bubble_width,
        bubble_height,
        (0, 0, 0, if bubble_theme.is_dark() { 82 } else { 38 }),
        (14.0 * scale).round().max(10.0) as u32,
    );
    let (fill, stroke) = if bubble_theme.is_dark() {
        ((24, 26, 31, 255), (255, 255, 255, 52))
    } else {
        ((255, 255, 255, 255), (36, 40, 48, 42))
    };
    fill_rounded_rect(
        buffer,
        width,
        bubble_x,
        bubble_y,
        bubble_width,
        bubble_height,
        fill,
        (14.0 * scale).round().max(10.0) as u32,
    );
    stroke_rounded_rect(
        buffer,
        width,
        bubble_x,
        bubble_y,
        bubble_width,
        bubble_height,
        stroke,
        (14.0 * scale).round().max(10.0) as u32,
    );
}

fn fill_rounded_rect(
    buffer: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    rect_width: u32,
    rect_height: u32,
    (r, g, b, a): (u8, u8, u8, u8),
    radius: u32,
) {
    for py in y..y.saturating_add(rect_height) {
        for px in x..x.saturating_add(rect_width) {
            let left = px.saturating_sub(x);
            let top = py.saturating_sub(y);
            let right = x.saturating_add(rect_width).saturating_sub(px + 1);
            let bottom = y.saturating_add(rect_height).saturating_sub(py + 1);
            let corner_dx = radius.saturating_sub(left.min(right).saturating_add(1));
            let corner_dy = radius.saturating_sub(top.min(bottom).saturating_add(1));
            if corner_dx > 0
                && corner_dy > 0
                && corner_dx * corner_dx + corner_dy * corner_dy > radius * radius
            {
                continue;
            }

            let index = ((py * width + px) * 4) as usize;
            if index + 3 >= buffer.len() {
                continue;
            }
            buffer[index] = ((u16::from(b) * u16::from(a) + 127) / 255) as u8;
            buffer[index + 1] = ((u16::from(g) * u16::from(a) + 127) / 255) as u8;
            buffer[index + 2] = ((u16::from(r) * u16::from(a) + 127) / 255) as u8;
            buffer[index + 3] = a;
        }
    }
}

fn stroke_rounded_rect(
    buffer: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    rect_width: u32,
    rect_height: u32,
    color: (u8, u8, u8, u8),
    radius: u32,
) {
    fill_rounded_rect(buffer, width, x, y, rect_width, 1, color, radius);
    fill_rounded_rect(
        buffer,
        width,
        x,
        y.saturating_add(rect_height.saturating_sub(1)),
        rect_width,
        1,
        color,
        radius,
    );
    fill_rounded_rect(buffer, width, x, y, 1, rect_height, color, radius);
    fill_rounded_rect(
        buffer,
        width,
        x.saturating_add(rect_width.saturating_sub(1)),
        y,
        1,
        rect_height,
        color,
        radius,
    );
}

fn paint_layered_window(
    window: &Window,
    bgra: &[u8],
    width: i32,
    height: i32,
    scale: f64,
    bubble_text: Option<&str>,
    bubble_theme: PetBubbleTheme,
) -> LocalResult<()> {
    unsafe {
        use windows_sys::Win32::{
            Foundation::POINT,
            Graphics::Gdi::{
                CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
                AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
                DIB_RGB_COLORS,
            },
            UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA},
        };

        let hwnd = window.hwnd().map_err(|err| err.to_string())?.0 as *mut std::ffi::c_void;
        let position = window.outer_position().map_err(|err| err.to_string())?;
        let screen_position = POINT {
            x: position.x,
            y: position.y,
        };
        let size = windows_sys::Win32::Foundation::SIZE {
            cx: width,
            cy: height,
        };
        let source_position = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let memory_dc = CreateCompatibleDC(std::ptr::null_mut());
        if memory_dc.is_null() {
            return Err("创建桌宠绘制 DC 失败。".into());
        }

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            memory_dc,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        if bitmap.is_null() {
            DeleteDC(memory_dc);
            return Err("创建桌宠位图失败。".into());
        }

        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
        let old_bitmap = SelectObject(memory_dc, bitmap);
        if let Some(text) = bubble_text.filter(|value| !value.trim().is_empty()) {
            draw_bubble_text(memory_dc, width, scale, text, bubble_theme);
        }
        let ok = UpdateLayeredWindow(
            hwnd,
            std::ptr::null_mut(),
            &screen_position,
            &size,
            memory_dc,
            &source_position,
            0,
            &blend,
            ULW_ALPHA,
        );
        SelectObject(memory_dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(memory_dc);

        if ok == 0 {
            return Err("更新桌宠窗口失败。".into());
        }
    }
}
