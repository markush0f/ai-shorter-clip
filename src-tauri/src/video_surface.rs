//! UI-thread-only native drawing surfaces. No decoded frames cross the IPC boundary.
use serde::Deserialize;

#[derive(Clone, Copy, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub visible: bool,
    pub pixel_ratio: f64,
    #[serde(default)]
    pub refresh: bool,
}
impl Bounds {
    pub fn validate(&self) -> Result<(), String> {
        if [self.x, self.y, self.width, self.height]
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 100_000.0)
            || self.width < 0.0
            || self.height < 0.0
            || !self.pixel_ratio.is_finite()
            || self.pixel_ratio <= 0.0
            || self.pixel_ratio > 16.0
        {
            return Err("Dimensiones de vídeo no válidas".into());
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use gtk::{glib::translate::ToGlibPtr, prelude::*};
    use std::cell::RefCell;
    thread_local! { static SURFACE: RefCell<Option<gtk::DrawingArea>> = const { RefCell::new(None) }; }
    #[link(name = "gdk-3")]
    unsafe extern "C" {
        fn gdk_x11_window_get_xid(window: *mut gdk::ffi::GdkWindow) -> std::ffi::c_ulong;
    }

    pub fn update(window: &tauri::WebviewWindow, bounds: Bounds) -> Result<usize, String> {
        if gdk::Display::default().is_none_or(|display| !display.type_().name().contains("X11")) {
            return Err("La integración de libVLC 3 en Linux necesita X11/XWayland. Cierra y abre la aplicación sin forzar GDK_BACKEND=wayland.".into());
        }
        let ratio = bounds.pixel_ratio / window.scale_factor().map_err(|e| e.to_string())?;
        let bounds = Bounds {
            x: bounds.x * ratio,
            y: bounds.y * ratio,
            width: bounds.width * ratio,
            height: bounds.height * ratio,
            ..bounds
        };
        SURFACE.with(|cell| {
            let mut surface = cell.borrow_mut();
            if surface.is_none() {
                let container = window.default_vbox().map_err(|e| e.to_string())?;
                let child = container
                    .children()
                    .last()
                    .cloned()
                    .ok_or("No se encontró la vista principal")?;
                container.remove(&child);
                let overlay = gtk::Overlay::new();
                overlay.set_hexpand(true);
                overlay.set_vexpand(true);
                overlay.add(&child);
                let drawing = gtk::DrawingArea::new();
                drawing.set_halign(gtk::Align::Start);
                drawing.set_valign(gtk::Align::Start);
                // GTK must not double-buffer a drawable owned by an external video renderer.
                unsafe {
                    gtk::ffi::gtk_widget_set_double_buffered(
                        drawing.as_ptr() as *mut gtk::ffi::GtkWidget,
                        0,
                    );
                }
                drawing.set_size_request(640, 360);
                drawing.connect_draw(|_, context| {
                    context.set_source_rgb(0.03, 0.04, 0.06);
                    let _ = context.paint();
                    gtk::glib::Propagation::Proceed
                });
                overlay.add_overlay(&drawing);
                container.pack_start(&overlay, true, true, 0);
                overlay.show_all();
                drawing.realize();
                let native = drawing
                    .window()
                    .ok_or("No se pudo crear la superficie nativa")?;
                if !native.ensure_native() {
                    return Err("No se pudo crear una ventana X11 para el vídeo".into());
                }
                native.resize(640, 360);
                drawing.hide();
                *surface = Some(drawing);
            }
            let drawing = surface.as_ref().unwrap();
            drawing.set_margin_start(bounds.x.max(0.0).round() as i32);
            drawing.set_margin_top(bounds.y.max(0.0).round() as i32);
            drawing.set_size_request(
                bounds.width.max(1.0).round() as i32,
                bounds.height.max(1.0).round() as i32,
            );
            drawing.set_visible(bounds.visible && bounds.width > 0.0 && bounds.height > 0.0);
            let native = drawing
                .window()
                .ok_or("La superficie de vídeo ya no existe")?;
            // SAFETY: The checked X11 GdkWindow is live and all GTK access is on its UI thread.
            Ok(unsafe { gdk_x11_window_get_xid(native.to_glib_none().0) } as usize)
        })
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::{cell::Cell, ptr::null_mut};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    thread_local! { static SURFACE: Cell<isize> = const { Cell::new(0) }; }
    pub fn update(window: &tauri::WebviewWindow, bounds: Bounds) -> Result<usize, String> {
        SURFACE.with(|cell| unsafe {
            if cell.get() == 0 {
                let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
                let child = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                    0,
                    0,
                    1,
                    1,
                    window.hwnd().map_err(|e| e.to_string())?.0 as _,
                    null_mut(),
                    null_mut(),
                    std::ptr::null(),
                );
                if child.is_null() {
                    return Err("No se pudo crear la superficie de vídeo de Windows".into());
                }
                cell.set(child as isize);
            }
            let handle = cell.get() as windows_sys::Win32::Foundation::HWND;
            let scale = bounds.pixel_ratio;
            SetWindowPos(
                handle,
                HWND_TOP,
                (bounds.x * scale).round() as i32,
                (bounds.y * scale).round() as i32,
                (bounds.width * scale).max(1.0).round() as i32,
                (bounds.height * scale).max(1.0).round() as i32,
                SWP_NOACTIVATE
                    | if bounds.visible {
                        SWP_SHOWWINDOW
                    } else {
                        SWP_HIDEWINDOW
                    },
            );
            Ok(handle as usize)
        })
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::{
        cell::Cell,
        ffi::{CString, c_char, c_void},
    };
    type Object = *mut c_void;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Object;
        fn sel_registerName(name: *const c_char) -> Object;
        fn objc_msgSend();
    }
    thread_local! { static SURFACE: Cell<usize> = const { Cell::new(0) }; }
    unsafe fn selector(name: &str) -> Object {
        unsafe { sel_registerName(CString::new(name).unwrap().as_ptr()) }
    }
    pub fn update(window: &tauri::WebviewWindow, bounds: Bounds) -> Result<usize, String> {
        SURFACE.with(|cell| unsafe {
            let send: unsafe extern "C" fn(Object, Object) -> Object =
                std::mem::transmute(objc_msgSend as *const ());
            if cell.get() == 0 {
                let class = objc_getClass(c"NSView".as_ptr());
                let allocated = send(class, selector("alloc"));
                let init: unsafe extern "C" fn(Object, Object, Rect) -> Object =
                    std::mem::transmute(objc_msgSend as *const ());
                let view = init(
                    allocated,
                    selector("initWithFrame:"),
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                );
                if view.is_null() {
                    return Err("No se pudo crear la superficie de vídeo de macOS".into());
                }
                let parent = send(
                    window.ns_window().map_err(|e| e.to_string())? as _,
                    selector("contentView"),
                );
                let add: unsafe extern "C" fn(Object, Object, Object) =
                    std::mem::transmute(objc_msgSend as *const ());
                add(parent, selector("addSubview:"), view);
                cell.set(view as usize);
                // The parent retains it; release our allocation ownership.
                let release: unsafe extern "C" fn(Object, Object) =
                    std::mem::transmute(objc_msgSend as *const ());
                release(view, selector("release"));
            }
            let height = window.inner_size().map_err(|e| e.to_string())?.height as f64
                / window.scale_factor().map_err(|e| e.to_string())?;
            let ratio = bounds.pixel_ratio / window.scale_factor().map_err(|e| e.to_string())?;
            let frame = Rect {
                x: bounds.x * ratio,
                y: height - (bounds.y + bounds.height) * ratio,
                width: (bounds.width * ratio).max(1.0),
                height: (bounds.height * ratio).max(1.0),
            };
            let set_frame: unsafe extern "C" fn(Object, Object, Rect) =
                std::mem::transmute(objc_msgSend as *const ());
            let set_hidden: unsafe extern "C" fn(Object, Object, bool) =
                std::mem::transmute(objc_msgSend as *const ());
            set_frame(cell.get() as Object, selector("setFrame:"), frame);
            set_hidden(
                cell.get() as Object,
                selector("setHidden:"),
                !bounds.visible,
            );
            Ok(cell.get())
        })
    }
}

pub async fn update(window: tauri::WebviewWindow, bounds: Bounds) -> Result<usize, String> {
    update_checked(window, bounds, || true).await
}

pub async fn update_checked(
    window: tauri::WebviewWindow,
    bounds: Bounds,
    current: impl FnOnce() -> bool + Send + 'static,
) -> Result<usize, String> {
    bounds.validate()?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let ui_window = window.clone();
    window
        .run_on_main_thread(move || {
            let result = if current() {
                platform::update(&ui_window, bounds)
            } else {
                Err("La selección de vídeo ha cambiado".into())
            };
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_surface_sizes() {
        let mut bounds = Bounds {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 360.0,
            visible: true,
            pixel_ratio: 1.0,
            refresh: false,
        };
        assert!(bounds.validate().is_ok());
        bounds.width = f64::NAN;
        assert!(bounds.validate().is_err());
        bounds.width = -1.0;
        assert!(bounds.validate().is_err());
        bounds.width = 640.0;
        bounds.y = f64::INFINITY;
        assert!(bounds.validate().is_err());
    }
}
