//! Per-process runtime setup; never changes the user's session or system packages.
#[cfg(target_os = "linux")]
pub fn prepare_process() {
    use std::{os::unix::process::CommandExt, path::PathBuf, process::Command};
    // A local development runtime or a packager-provided sibling `vlc/` directory.
    let mut roots = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        roots.push(parent.join("vlc"));
    }
    #[cfg(debug_assertions)]
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.runtime/vlc"));
    if std::env::var_os("AI_SHORTER_VLC_RUNTIME_READY").is_none()
        && let Some(root) = roots
            .into_iter()
            .find(|root| root.join("usr/lib64/libvlc.so.5").is_file())
    {
        let root = root.canonicalize().expect("local VLC runtime exists");
        let mut paths = vec![root.join("usr/lib64/ffmpeg"), root.join("usr/lib64")];
        if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        // glibc reads LD_LIBRARY_PATH at exec time, so mutating it after main is insufficient.
        // Re-exec once with a process-local loader path; no shell configuration is touched.
        let error = Command::new(std::env::current_exe().expect("application executable"))
            .args(std::env::args_os().skip(1))
            .env(
                "LD_LIBRARY_PATH",
                std::env::join_paths(paths).expect("valid VLC runtime paths"),
            )
            .env("VLC_PLUGIN_PATH", root.join("usr/lib64/vlc/plugins"))
            .env("AI_SHORTER_VLC_LIBRARY", root.join("usr/lib64/libvlc.so.5"))
            .env("AI_SHORTER_VLC_RUNTIME_READY", "1")
            .exec();
        eprintln!("No se pudo iniciar el runtime local de VLC: {error}");
        std::process::exit(1);
    }
    // SAFETY: main calls this before Tauri or any worker threads are started.
    unsafe {
        if std::env::var_os("GDK_BACKEND").is_none() && std::env::var_os("DISPLAY").is_some() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // Must precede GTK's first XOpenDisplay (LibVLC's documented embedding requirement).
        #[link(name = "X11")]
        unsafe extern "C" {
            fn XInitThreads() -> std::ffi::c_int;
        }
        if XInitThreads() == 0 {
            eprintln!("Aviso: X11 no pudo inicializar el soporte multihilo");
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn prepare_process() {}
