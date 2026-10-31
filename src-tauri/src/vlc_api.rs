//! Small, typed LibVLC 3 C-ABI boundary. The library outlives every function pointer.
use libloading::Library;
use std::{
    ffi::{CStr, c_char, c_int, c_uint, c_void},
    path::PathBuf,
};

pub type Handle = *mut c_void;

// LibVLC 3's public libvlc_media_track_t layout. The details union holds a pointer.
#[repr(C)]
pub struct MediaTrack {
    pub codec: u32,
    pub original_fourcc: u32,
    pub id: c_int,
    pub kind: c_int,
    pub profile: c_int,
    pub level: c_int,
    pub details: *mut c_void,
    pub bitrate: u32,
    pub language: *mut c_char,
    pub description: *mut c_char,
}
// Exact LibVLC 3 libvlc_media_stats_t layout, including legacy stream-output fields.
#[repr(C)]
#[derive(Default)]
pub struct MediaStats {
    pub read_bytes: c_int,
    pub input_bitrate: f32,
    pub demux_read_bytes: c_int,
    pub demux_bitrate: f32,
    pub demux_corrupted: c_int,
    pub demux_discontinuity: c_int,
    pub decoded_video: c_int,
    pub decoded_audio: c_int,
    pub displayed_pictures: c_int,
    pub lost_pictures: c_int,
    pub played_audio_buffers: c_int,
    pub lost_audio_buffers: c_int,
    pub sent_packets: c_int,
    pub sent_bytes: c_int,
    pub send_bitrate: f32,
}

macro_rules! api {
    ($($name:ident: ($($arg:ty),*) -> $result:ty),* $(,)?) => {
        pub struct Api {
            $(pub $name: unsafe extern "C" fn($($arg),*) -> $result,)*
            library: Library,
        }
        impl Api {
            pub unsafe fn load(path: &std::path::Path) -> Result<Self, String> {
                // SAFETY: Only the explicitly configured/bundled or system LibVLC is loaded.
                let library = unsafe { Library::new(path) }.map_err(|e| e.to_string())?;
                Ok(Self {
                    $($name: unsafe { *library.get(concat!("libvlc_", stringify!($name), "\0").as_bytes()).map_err(|e| e.to_string())? },)*
                    library,
                })
            }
        }
    }
}
api! {
    new: (c_int, *const *const c_char) -> Handle,
    release: (Handle) -> (),
    get_version: () -> *const c_char,
    media_new_path: (Handle, *const c_char) -> Handle,
    media_release: (Handle) -> (),
    media_add_option: (Handle, *const c_char) -> (),
    media_get_duration: (Handle) -> i64,
    media_get_stats: (Handle, *mut MediaStats) -> c_int,
    media_tracks_get: (Handle, *mut *mut *mut MediaTrack) -> c_uint,
    media_tracks_release: (*mut *mut MediaTrack, c_uint) -> (),
    media_player_new: (Handle) -> Handle,
    media_player_release: (Handle) -> (),
    media_player_set_media: (Handle, Handle) -> (),
    media_player_play: (Handle) -> c_int,
    media_player_set_pause: (Handle, c_int) -> (),
    media_player_stop: (Handle) -> (),
    media_player_get_state: (Handle) -> c_int,
    media_player_get_length: (Handle) -> i64,
    media_player_get_time: (Handle) -> i64,
    media_player_set_time: (Handle, i64) -> (),
    media_player_is_seekable: (Handle) -> c_int,
    video_set_mouse_input: (Handle, c_uint) -> (),
    video_set_key_input: (Handle, c_uint) -> (),
    video_get_size: (Handle, c_uint, *mut c_uint, *mut c_uint) -> c_int,
    audio_set_volume: (Handle, c_int) -> c_int,
    audio_set_mute: (Handle, c_int) -> (),
}

impl Api {
    pub fn discover() -> Result<Self, String> {
        let mut paths = Vec::new();
        if let Some(path) = std::env::var_os("AI_SHORTER_VLC_LIBRARY") {
            paths.push(PathBuf::from(path));
        }
        #[cfg(target_os = "linux")]
        paths.push(PathBuf::from("libvlc.so.5"));
        #[cfg(target_os = "windows")]
        {
            if let Some(base) = std::env::var_os("ProgramFiles") {
                paths.push(PathBuf::from(base).join("VideoLAN/VLC/libvlc.dll"));
            }
            paths.push(PathBuf::from("libvlc.dll"));
        }
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(
                "/Applications/VLC.app/Contents/MacOS/lib/libvlc.dylib",
            ));
            paths.push(PathBuf::from("libvlc.dylib"));
        }
        let mut last_error = String::new();
        for path in paths {
            match unsafe { Self::load(&path) } {
                Ok(api) => {
                    let version = api.version();
                    if !version.starts_with("3.") {
                        return Err(format!("Se necesita libVLC 3.x; se encontró {version}."));
                    }
                    return Ok(api);
                }
                Err(error) => last_error = error,
            }
        }
        Err(format!(
            "No se encontró libVLC 3. En Fedora ejecuta npm run setup:vlc; en otros sistemas instala VLC 3 de 64 bits. Detalle: {last_error}"
        ))
    }

    pub fn version(&self) -> String {
        // SAFETY: LibVLC returns an immutable, NUL-terminated string valid for the library lifetime.
        unsafe {
            CStr::from_ptr((self.get_version)())
                .to_string_lossy()
                .into_owned()
        }
    }

    pub unsafe fn attach(&self, player: Handle, surface: usize) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let set: libloading::Symbol<unsafe extern "C" fn(Handle, u32)> =
                unsafe { self.library.get(b"libvlc_media_player_set_xwindow\0") }
                    .map_err(|e| e.to_string())?;
            unsafe { set(player, surface as u32) };
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            #[cfg(target_os = "windows")]
            let symbol = b"libvlc_media_player_set_hwnd\0".as_slice();
            #[cfg(target_os = "macos")]
            let symbol = b"libvlc_media_player_set_nsobject\0".as_slice();
            let set: libloading::Symbol<unsafe extern "C" fn(Handle, Handle)> =
                unsafe { self.library.get(symbol) }.map_err(|e| e.to_string())?;
            unsafe { set(player, surface as Handle) };
        }
        Ok(())
    }
}
