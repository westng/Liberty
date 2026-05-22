use super::drag::{DragPoint, PetDragMachine, PetDragMove};
use super::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use tauri::PhysicalPosition;
use windows_sys::Win32::Foundation::POINT;

struct PetWindowInputState {
    diagnostic: Arc<PetWindowDiagnostic>,
    interaction_signal: Arc<AtomicU64>,
    drag_state: Mutex<PetWindowDragState>,
}

#[derive(Default)]
struct PetWindowDragState {
    machine: PetDragMachine,
}

struct PetWindowDiagnostic {
    app: AppHandle,
    label: String,
    message_count: AtomicU64,
    move_count: AtomicU64,
}

impl PetWindowDiagnostic {
    fn log(&self, line: impl AsRef<str>) {
        append_diagnostic(&self.app, format!("{} {}", self.label, line.as_ref()));
    }
}

pub fn prepare_window(window: &Window, interaction_signal: &Arc<AtomicU64>) -> LocalResult<()> {
    set_native_window_style(window, interaction_signal)
}

pub fn paint_window(
    window: &Window,
    frame_path: &Path,
    bubble_text: Option<&str>,
    growth_float: Option<&PetGrowthFloat>,
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
    if let Some(growth_float) = growth_float {
        draw_growth_glow(&mut buffer, window_width, scale, growth_float);
    }

    let _ = window.set_size(tauri::LogicalSize::new(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT));
    paint_layered_window(
        window,
        &buffer,
        window_width as i32,
        window_height as i32,
        scale,
        bubble_text,
        growth_float,
        bubble_theme,
    )
}

fn set_native_window_style(
    window: &Window,
    interaction_signal: &Arc<AtomicU64>,
) -> LocalResult<()> {
    let hwnd = window.hwnd().map_err(|err| err.to_string())?;
    let diagnostic = Arc::new(PetWindowDiagnostic {
        app: window.app_handle().clone(),
        label: window.label().to_string(),
        message_count: AtomicU64::new(0),
        move_count: AtomicU64::new(0),
    });
    let input_state = Box::new(PetWindowInputState {
        diagnostic: diagnostic.clone(),
        interaction_signal: interaction_signal.clone(),
        drag_state: Mutex::new(PetWindowDragState::default()),
    });
    unsafe {
        use windows_sys::Win32::UI::{
            Shell::SetWindowSubclass,
            WindowsAndMessaging::{
                GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE,
                WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            },
        };
        let hwnd = hwnd.0 as *mut std::ffi::c_void;
        windows_sys::Win32::Foundation::SetLastError(0);
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        diagnostic.log(format!("style before exstyle=0x{style:x}"));
        if style == 0 {
            let last_error = windows_sys::Win32::Foundation::GetLastError();
            if last_error != 0 {
                return Err(format!(
                    "读取 Windows 桌宠窗口样式失败，Win32 error={last_error}"
                ));
            }
        }
        #[cfg(target_pointer_width = "64")]
        let next_style = (style
            | WS_EX_LAYERED as isize
            | WS_EX_TOOLWINDOW as isize
            | WS_EX_NOACTIVATE as isize)
            & !(WS_EX_TRANSPARENT as isize);
        #[cfg(target_pointer_width = "32")]
        let next_style =
            (style | WS_EX_LAYERED as i32 | WS_EX_TOOLWINDOW as i32 | WS_EX_NOACTIVATE as i32)
                & !(WS_EX_TRANSPARENT as i32);
        windows_sys::Win32::Foundation::SetLastError(0);
        let previous_style = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_style);
        diagnostic.log(format!(
            "style set previous=0x{previous_style:x} next=0x{next_style:x}"
        ));
        if previous_style == 0 {
            let last_error = windows_sys::Win32::Foundation::GetLastError();
            if last_error != 0 {
                return Err(format!(
                    "设置 Windows 桌宠窗口样式失败，Win32 error={last_error}"
                ));
            }
        }
        let input_state = Box::into_raw(input_state) as usize;
        let subclassed = SetWindowSubclass(hwnd, Some(pet_window_subclass_proc), 1, input_state);
        if subclassed == 0 {
            drop(Box::from_raw(input_state as *mut PetWindowInputState));
            return Err(format!(
                "安装 Windows 桌宠拖拽消息钩子失败，Win32 error={}",
                windows_sys::Win32::Foundation::GetLastError()
            ));
        }
        diagnostic.log(format!("subclass installed hwnd={hwnd:p}"));
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
            HTCLIENT, WM_CAPTURECHANGED, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
            WM_NCHITTEST,
        },
    };

    let input_state = (ref_data != 0).then(|| &*(ref_data as *const PetWindowInputState));

    if msg == WM_NCHITTEST {
        if let Some(input_state) = input_state {
            input_state
                .diagnostic
                .log("message WM_NCHITTEST -> HTCLIENT");
        }
        return HTCLIENT as isize;
    }

    match msg {
        WM_LBUTTONDOWN => {
            if let Some(input_state) = input_state {
                input_state.diagnostic.log("message WM_LBUTTONDOWN");
                record_mouse_down(hwnd, input_state);
            }
        }
        WM_MOUSEMOVE => {
            if let Some(input_state) = input_state {
                let count = input_state
                    .diagnostic
                    .message_count
                    .fetch_add(1, Ordering::Relaxed);
                if count < 8 || count % 40 == 0 {
                    input_state
                        .diagnostic
                        .log(format!("message WM_MOUSEMOVE count={}", count + 1));
                }
                drag_window(hwnd, input_state);
            }
        }
        WM_LBUTTONUP => {
            if let Some(input_state) = input_state {
                input_state.diagnostic.log("message WM_LBUTTONUP");
                finish_mouse_interaction(input_state);
            }
        }
        WM_CAPTURECHANGED => {
            if let Some(input_state) = input_state {
                input_state.diagnostic.log("message WM_CAPTURECHANGED");
                reset_drag_state(input_state);
            }
        }
        WM_DESTROY => {
            if ref_data != 0 {
                if let Some(input_state) = input_state {
                    input_state.diagnostic.log("message WM_DESTROY");
                }
                RemoveWindowSubclass(hwnd, Some(pet_window_subclass_proc), _subclass_id);
                drop(Box::from_raw(ref_data as *mut PetWindowInputState));
            }
        }
        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe fn record_mouse_down(hwnd: *mut std::ffi::c_void, input_state: &PetWindowInputState) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetCapture, SetCapture};

    let captured = SetCapture(hwnd);
    let active_capture = GetCapture();
    if let Ok(mut guard) = input_state.drag_state.lock() {
        if let (Some(cursor), Some(window)) = (cursor_position(), window_position(hwnd)) {
            input_state.diagnostic.log(format!(
                "mouse down cursor=({}, {}) window=({}, {}) set_capture_previous={captured:p} active_capture={active_capture:p}",
                cursor.x, cursor.y, window.x, window.y
            ));
            guard
                .machine
                .record_mouse_down(cursor_drag_point(cursor), window_drag_point(window));
        } else {
            input_state
                .diagnostic
                .log("mouse down failed to read cursor/window position");
            guard.machine.reset();
        }
    }
}

unsafe fn next_drag_move(input_state: &PetWindowInputState) -> Option<DragPoint> {
    let Some(current) = cursor_position() else {
        return None;
    };

    let Ok(mut guard) = input_state.drag_state.lock() else {
        return None;
    };

    match guard.machine.pointer_moved(cursor_drag_point(current)) {
        PetDragMove::None => None,
        PetDragMove::MoveTo(position) => Some(position),
    }
}

unsafe fn drag_window(hwnd: *mut std::ffi::c_void, input_state: &PetWindowInputState) {
    let Some(position) = next_drag_move(input_state) else {
        return;
    };
    let count = input_state
        .diagnostic
        .move_count
        .fetch_add(1, Ordering::Relaxed);
    if count < 8 || count % 20 == 0 {
        input_state.diagnostic.log(format!(
            "drag move requested count={} target=({}, {})",
            count + 1,
            position.x,
            position.y
        ));
    }
    if let Err(error) = move_window_from_drag(hwnd, position) {
        input_state
            .diagnostic
            .log(format!("drag move SetWindowPos failed: {error}"));
        begin_system_window_drag(hwnd);
    } else if count < 8 || count % 20 == 0 {
        if let Some(window) = window_position(hwnd) {
            input_state.diagnostic.log(format!(
                "drag move SetWindowPos ok count={} actual=({}, {})",
                count + 1,
                window.x,
                window.y
            ));
        }
    }
}

unsafe fn move_window_from_drag(
    hwnd: *mut std::ffi::c_void,
    position: DragPoint,
) -> LocalResult<()> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };

    let moved = SetWindowPos(
        hwnd,
        std::ptr::null_mut(),
        position.x,
        position.y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    if moved == 0 {
        let last_error = windows_sys::Win32::Foundation::GetLastError();
        return Err(format!("移动桌宠窗口失败，Win32 error={last_error}"));
    }
    Ok(())
}

fn finish_mouse_interaction(input_state: &PetWindowInputState) {
    let should_interact = input_state
        .drag_state
        .lock()
        .map(|mut guard| guard.machine.finish_mouse_interaction())
        .unwrap_or(false);

    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture};

        let active_capture = GetCapture();
        input_state
            .diagnostic
            .log(format!("finish interaction active_capture={active_capture:p} should_interact={should_interact}"));
        ReleaseCapture();
    }

    if should_interact {
        input_state
            .interaction_signal
            .fetch_add(1, Ordering::Relaxed);
    }
}

fn reset_drag_state(input_state: &PetWindowInputState) {
    if let Ok(mut guard) = input_state.drag_state.lock() {
        guard.machine.reset();
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

unsafe fn window_position(hwnd: *mut std::ffi::c_void) -> Option<PhysicalPosition<i32>> {
    use windows_sys::Win32::{Foundation::RECT, UI::WindowsAndMessaging::GetWindowRect};

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return None;
    }
    Some(PhysicalPosition::new(rect.left, rect.top))
}

fn cursor_drag_point(point: POINT) -> DragPoint {
    DragPoint::new(point.x, point.y)
}

fn window_drag_point(position: PhysicalPosition<i32>) -> DragPoint {
    DragPoint::new(position.x, position.y)
}

unsafe fn begin_system_window_drag(hwnd: *mut std::ffi::c_void) {
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{SendMessageW, HTCAPTION, WM_NCLBUTTONDOWN},
    };

    ReleaseCapture();
    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::c_void, ptr};
    use windows_sys::Win32::{
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowRect, RegisterClassW,
            SetCursorPos, CS_HREDRAW, CS_VREDRAW, WNDCLASSW, WS_OVERLAPPED,
        },
    };

    unsafe extern "system" fn test_window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    #[test]
    fn drag_window_moves_real_win32_window_after_threshold() {
        unsafe {
            let class_name = wide_null("LibertyDesktopPetDragTestWindow");
            let window_name = wide_null("Liberty Desktop Pet Drag Test");
            let window_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(test_window_proc),
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&window_class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_name.as_ptr(),
                WS_OVERLAPPED,
                40,
                50,
                160,
                120,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            assert!(
                !hwnd.is_null(),
                "CreateWindowExW failed: {}",
                GetLastError()
            );

            let input_state = PetWindowInputState {
                interaction_signal: Arc::new(AtomicU64::new(0)),
                drag_state: Mutex::new(PetWindowDragState::default()),
            };

            assert_ne!(SetCursorPos(100, 100), 0, "SetCursorPos start failed");
            record_mouse_down(hwnd as *mut c_void, &input_state);
            assert_ne!(SetCursorPos(130, 125), 0, "SetCursorPos move failed");
            drag_window(hwnd as *mut c_void, &input_state);

            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            assert_ne!(GetWindowRect(hwnd, &mut rect), 0, "GetWindowRect failed");
            assert_eq!(rect.left, 70);
            assert_eq!(rect.top, 75);

            finish_mouse_interaction(&input_state);
            DestroyWindow(hwnd);
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
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

fn draw_growth_glow(buffer: &mut [u8], width: u32, scale: f64, growth_float: &PetGrowthFloat) {
    let (x, y, alpha) = growth_float_rect(scale, growth_float);
    let alpha = (alpha * 46.0).round().clamp(0.0, 46.0) as u8;
    fill_rounded_rect(
        buffer,
        width,
        x.max(0) as u32,
        y.max(0) as u32,
        (72.0 * scale).round().max(1.0) as u32,
        (30.0 * scale).round().max(1.0) as u32,
        (255, 122, 68, alpha),
        (15.0 * scale).round().max(8.0) as u32,
    );
}

fn growth_float_rect(scale: f64, growth_float: &PetGrowthFloat) -> (i32, i32, f64) {
    let elapsed_ms = growth_float
        .started_at
        .elapsed()
        .unwrap_or_default()
        .as_millis()
        .min(3_000) as f64;
    let progress = elapsed_ms / 3_000.0;
    let alpha = if progress < 0.72 {
        1.0
    } else {
        ((1.0 - progress) / 0.28).clamp(0.0, 1.0)
    };
    let x = (196.0 * scale).round() as i32;
    let y = ((102.0 - progress * 34.0) * scale).round() as i32;
    (x, y, alpha)
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
    growth_float: Option<&PetGrowthFloat>,
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
            let text_rect = draw_bubble_text(memory_dc, width, scale, text, bubble_theme);
            normalize_bubble_text_alpha(bits.cast::<u8>(), width, height, text_rect);
        }
        if let Some(growth_float) = growth_float {
            let text_rect = draw_growth_text(memory_dc, scale, growth_float);
            normalize_bubble_text_alpha(bits.cast::<u8>(), width, height, text_rect);
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

        Ok(())
    }
}

unsafe fn draw_bubble_text(
    memory_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    width: i32,
    scale: f64,
    text: &str,
    bubble_theme: PetBubbleTheme,
) -> windows_sys::Win32::Foundation::RECT {
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            CreateFontW, DeleteObject, DrawTextW, SelectObject, SetBkMode, SetTextColor, DT_LEFT,
            DT_SINGLELINE, DT_VCENTER, DT_WORD_ELLIPSIS, FW_SEMIBOLD, TRANSPARENT,
        },
    };

    let face_name = wide_null("Segoe UI");
    let font_height = -((13.0 * scale).round().max(11.0) as i32);
    let font = CreateFontW(
        font_height,
        0,
        0,
        0,
        FW_SEMIBOLD as i32,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        face_name.as_ptr(),
    );
    let old_font = if font.is_null() {
        std::ptr::null_mut()
    } else {
        SelectObject(memory_dc, font)
    };

    SetBkMode(memory_dc, TRANSPARENT as i32);
    let text_color = if bubble_theme.is_dark() {
        color_ref(245, 245, 247)
    } else {
        color_ref(20, 21, 26)
    };
    SetTextColor(memory_dc, text_color);

    let left = (36.0 * scale).round() as i32;
    let top = (8.0 * scale).round() as i32;
    let right = width.saturating_sub((36.0 * scale).round() as i32);
    let bottom = top + (64.0 * scale).round().max(1.0) as i32;
    let mut rect = RECT {
        left,
        top,
        right,
        bottom,
    };
    let wide_text = wide_null(text);
    DrawTextW(
        memory_dc,
        wide_text.as_ptr(),
        text.encode_utf16().count() as i32,
        &mut rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_WORD_ELLIPSIS,
    );

    if !old_font.is_null() {
        SelectObject(memory_dc, old_font);
    }
    if !font.is_null() {
        DeleteObject(font);
    }

    rect
}

unsafe fn draw_growth_text(
    memory_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    scale: f64,
    growth_float: &PetGrowthFloat,
) -> windows_sys::Win32::Foundation::RECT {
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            CreateFontW, DeleteObject, DrawTextW, SelectObject, SetBkMode, SetTextColor, DT_CENTER,
            DT_SINGLELINE, DT_VCENTER, FW_BOLD, TRANSPARENT,
        },
    };

    let face_name = wide_null("Segoe UI");
    let font_height = -((18.0 * scale).round().max(15.0) as i32);
    let font = CreateFontW(
        font_height,
        0,
        0,
        0,
        FW_BOLD as i32,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        face_name.as_ptr(),
    );
    let old_font = if font.is_null() {
        std::ptr::null_mut()
    } else {
        SelectObject(memory_dc, font)
    };

    let (x, y, _) = growth_float_rect(scale, growth_float);
    SetBkMode(memory_dc, TRANSPARENT as i32);
    SetTextColor(memory_dc, color_ref(255, 104, 58));

    let mut rect = RECT {
        left: x,
        top: y,
        right: x + (72.0 * scale).round() as i32,
        bottom: y + (30.0 * scale).round() as i32,
    };
    let text = format!("+{}", growth_float.value);
    let wide_text = wide_null(&text);
    DrawTextW(
        memory_dc,
        wide_text.as_ptr(),
        text.encode_utf16().count() as i32,
        &mut rect,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );

    if !old_font.is_null() {
        SelectObject(memory_dc, old_font);
    }
    if !font.is_null() {
        DeleteObject(font);
    }

    rect
}

unsafe fn normalize_bubble_text_alpha(
    bits: *mut u8,
    width: i32,
    height: i32,
    rect: windows_sys::Win32::Foundation::RECT,
) {
    if bits.is_null() || width <= 0 || height <= 0 {
        return;
    }

    let left = rect.left.clamp(0, width);
    let top = rect.top.clamp(0, height);
    let right = rect.right.clamp(left, width);
    let bottom = rect.bottom.clamp(top, height);

    for y in top..bottom {
        for x in left..right {
            let index = ((y * width + x) * 4) as isize;
            let blue = *bits.offset(index);
            let green = *bits.offset(index + 1);
            let red = *bits.offset(index + 2);
            if red != 0 || green != 0 || blue != 0 {
                *bits.offset(index + 3) = 255;
            }
        }
    }
}

fn color_ref(red: u8, green: u8, blue: u8) -> u32 {
    u32::from(red) | (u32::from(green) << 8) | (u32::from(blue) << 16)
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
