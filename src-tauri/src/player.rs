//! Native playback: scoped files, serialized LibVLC calls, and a backend segment guard.
use crate::{
    video_surface::{self, Bounds},
    vlc_api::{Api, Handle},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::CString,
    fs::File,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Manager, State};

#[derive(Clone, Default)]
pub struct PlayerState(pub Arc<Mutex<Option<Player>>>);
impl PlayerState {
    pub fn start_guard(&self) {
        let state = Arc::downgrade(&self.0);
        std::thread::spawn(move || {
            loop {
                {
                    let Some(state) = state.upgrade() else { break };
                    if let Ok(mut guard) = state.lock()
                        && let Some(player) = guard.as_mut()
                    {
                        player.tick();
                    }
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });
    }
    pub fn shutdown(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.take();
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub version: String,
    pub state: &'static str,
    pub duration: Option<f64>,
    pub position: f64,
    pub playing: bool,
    pub seekable: bool,
    pub volume: u32,
    pub muted: bool,
    pub width: u32,
    pub height: u32,
    pub segment_active: bool,
    pub decoded_frames: u32,
    pub displayed_frames: u32,
    pub lost_frames: u32,
}

pub struct Player {
    api: Api,
    instance: Handle,
    player: Handle,
    media: Handle,
    id: String,
    duration: i64,
    segment_end: Option<i64>,
    frozen_position: Option<i64>,
    pending_seek: Option<i64>,
    volume: u32,
    muted: bool,
    has_video: Option<bool>,
    audio_dirty: bool,
    surface_visible: bool,
    poster_pending: bool,
}
// SAFETY: LibVLC permits calls from application threads. Every access to these handles,
// including destruction, is serialized by PlayerState's mutex; GTK objects never enter it.
unsafe impl Send for Player {}

impl Player {
    fn new() -> Result<Self, String> {
        Self::with_options(&[])
    }
    fn with_options(extra: &[&std::ffi::CStr]) -> Result<Self, String> {
        let api = Api::discover()?;
        let args = [
            c"--no-video-title-show",
            c"--no-snapshot-preview",
            c"--avcodec-hw=any",
            c"--no-metadata-network-access",
            c"--ignore-config",
            c"--stats",
        ];
        let pointers: Vec<_> = args
            .iter()
            .chain(extra.iter())
            .map(|arg| arg.as_ptr())
            .collect();
        // SAFETY: C strings and the pointer array remain live during initialization.
        let instance = unsafe { (api.new)(pointers.len() as i32, pointers.as_ptr()) };
        if instance.is_null() {
            return Err(
                "libVLC no pudo inicializarse. Comprueba sus módulos y dependencias.".into(),
            );
        }
        let player = unsafe { (api.media_player_new)(instance) };
        if player.is_null() {
            unsafe { (api.release)(instance) };
            return Err("libVLC no pudo crear el reproductor".into());
        }
        unsafe {
            (api.video_set_mouse_input)(player, 0);
            (api.video_set_key_input)(player, 0);
        }
        Ok(Self {
            api,
            instance,
            player,
            media: std::ptr::null_mut(),
            id: String::new(),
            duration: 0,
            segment_end: None,
            frozen_position: None,
            pending_seek: None,
            volume: 100,
            muted: false,
            has_video: None,
            audio_dirty: true,
            surface_visible: false,
            poster_pending: true,
        })
    }
    fn clear(&mut self) {
        unsafe {
            (self.api.media_player_stop)(self.player);
            (self.api.media_player_set_media)(self.player, std::ptr::null_mut());
            if !self.media.is_null() {
                (self.api.media_release)(self.media);
            }
        }
        self.media = std::ptr::null_mut();
        self.id.clear();
        self.duration = 0;
        self.has_video = None;
        self.surface_visible = false;
        self.poster_pending = true;
        self.segment_end = None;
        self.frozen_position = None;
        self.pending_seek = None;
    }
    fn load(&mut self, path: &Path, surface: usize) -> Result<Snapshot, String> {
        let path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| "La ruta contiene caracteres no válidos")?;
        self.clear();
        unsafe {
            self.api.attach(self.player, surface)?;
            self.media = (self.api.media_new_path)(self.instance, path.as_ptr());
            if self.media.is_null() {
                return Err("libVLC no pudo abrir el vídeo seleccionado".into());
            }
            (self.api.media_add_option)(self.media, c":start-paused".as_ptr());
            (self.api.media_player_set_media)(self.player, self.media);
            // Playback's paused preroll already parses tracks/duration. Avoid a second
            // independent parser reading the same (potentially very large) local file.
            if (self.api.media_player_play)(self.player) != 0 {
                self.clear();
                return Err(
                    "libVLC no pudo iniciar el vídeo. Comprueba los módulos de códec y salida."
                        .into(),
                );
            }
            self.audio_dirty = (self.api.audio_set_volume)(self.player, self.volume as i32) != 0;
            (self.api.audio_set_mute)(self.player, self.muted as i32);
        }
        self.id = uuid::Uuid::new_v4().to_string();
        Ok(self.snapshot())
    }
    fn matches(&self, id: &str) -> Result<(), String> {
        if self.id.is_empty() || self.id != id {
            Err("El vídeo ya no está abierto. Vuelve a seleccionarlo.".into())
        } else {
            Ok(())
        }
    }
    fn tick(&mut self) {
        if self.id.is_empty() {
            return;
        }
        unsafe {
            if self.poster_pending && self.surface_visible {
                let state = (self.api.media_player_get_state)(self.player);
                if state == 4 && (self.api.media_player_is_seekable)(self.player) != 0 {
                    // Refresh a paused native surface without autoplay, audio, JS frame copies,
                    // or advancing the visible timestamp after a resize/show.
                    let position = self
                        .frozen_position
                        .unwrap_or_else(|| (self.api.media_player_get_time)(self.player).max(0));
                    (self.api.media_player_set_time)(self.player, position);
                    self.poster_pending = false;
                } else if state == 3 {
                    self.poster_pending = false;
                }
            }
            // Audio output may not exist until the first frame is played. Keep the user's
            // desired settings and apply them when ready instead of failing on silent videos.
            if self.audio_dirty && (self.api.audio_set_volume)(self.player, self.volume as i32) == 0
            {
                (self.api.audio_set_mute)(self.player, self.muted as i32);
                self.audio_dirty = false;
            }
            if let Some(position) = self.pending_seek
                && (self.api.media_player_is_seekable)(self.player) != 0
            {
                (self.api.media_player_set_time)(self.player, position);
                self.pending_seek = None;
                if (self.api.media_player_get_state)(self.player) == 3 {
                    self.frozen_position = None;
                }
            }
            if let Some(end) = self.segment_end {
                let state = (self.api.media_player_get_state)(self.player);
                if (self.api.media_player_get_time)(self.player) >= end || state == 6 {
                    (self.api.media_player_set_pause)(self.player, 1);
                    (self.api.media_player_set_time)(self.player, end);
                    self.segment_end = None;
                    self.frozen_position = Some(end);
                } else if state == 7 {
                    self.segment_end = None;
                }
            }
        }
    }
    fn snapshot(&mut self) -> Snapshot {
        unsafe {
            let state = (self.api.media_player_get_state)(self.player);
            let length =
                (self.api.media_player_get_length)(self.player).max(if self.media.is_null() {
                    0
                } else {
                    (self.api.media_get_duration)(self.media)
                });
            if length > 0 {
                self.duration = length;
            }
            if self.has_video.is_none() && !self.media.is_null() {
                let mut tracks = std::ptr::null_mut();
                let count = (self.api.media_tracks_get)(self.media, &mut tracks);
                if count > 0 && !tracks.is_null() {
                    self.has_video = Some(
                        std::slice::from_raw_parts(tracks, count as usize)
                            .iter()
                            .any(|track| !track.is_null() && (**track).kind == 1),
                    );
                    (self.api.media_tracks_release)(tracks, count);
                }
            }
            let mut width = 0;
            let mut height = 0;
            (self.api.video_get_size)(self.player, 0, &mut width, &mut height);
            let position = self.frozen_position.unwrap_or_else(|| {
                if state == 6 {
                    self.duration
                } else {
                    (self.api.media_player_get_time)(self.player).max(0)
                }
            });
            let mut stats = crate::vlc_api::MediaStats::default();
            if !self.media.is_null() {
                (self.api.media_get_stats)(self.media, &mut stats);
            }
            Snapshot {
                id: self.id.clone(),
                version: self.api.version(),
                state: if self.has_video == Some(false) {
                    "error"
                } else {
                    match state {
                        0 => "idle",
                        1 => "opening",
                        2 => "buffering",
                        3 => "playing",
                        4 => "paused",
                        5 => "stopped",
                        6 => "ended",
                        _ => "error",
                    }
                },
                duration: (self.duration > 0).then_some(self.duration as f64 / 1000.0),
                position: position as f64 / 1000.0,
                playing: state == 3,
                seekable: (self.api.media_player_is_seekable)(self.player) != 0,
                volume: self.volume,
                muted: self.muted,
                width,
                height,
                segment_active: self.segment_end.is_some(),
                decoded_frames: stats.decoded_video.max(0) as u32,
                displayed_frames: stats.displayed_pictures.max(0) as u32,
                lost_frames: stats.lost_pictures.max(0) as u32,
            }
        }
    }
    fn control(&mut self, command: Control) -> Result<Snapshot, String> {
        unsafe {
            match command {
                Control::Play => {
                    self.segment_end = None;
                    self.frozen_position = None;
                    let state = (self.api.media_player_get_state)(self.player);
                    if state == 6 || state == 5 {
                        self.pending_seek = Some(0);
                        if (self.api.media_player_play)(self.player) != 0 {
                            return Err("No se pudo reanudar el vídeo".into());
                        }
                    }
                    (self.api.media_player_set_pause)(self.player, 0);
                }
                Control::Pause => {
                    self.segment_end = None;
                    (self.api.media_player_set_pause)(self.player, 1);
                }
                Control::Seek { seconds } => {
                    let time = seek_time(seconds, self.duration)?;
                    self.segment_end = None;
                    self.frozen_position = Some(time);
                    self.pending_seek = Some(time);
                    if (self.api.media_player_is_seekable)(self.player) != 0 {
                        (self.api.media_player_set_time)(self.player, time);
                        self.pending_seek = None;
                        // Freeze only when paused; playback should immediately follow the clock.
                        if (self.api.media_player_get_state)(self.player) == 3 {
                            self.frozen_position = None;
                        }
                    }
                }
                Control::Volume { percent } => {
                    if percent > 100 {
                        return Err("El volumen debe estar entre 0 y 100".into());
                    }
                    self.volume = percent;
                    self.audio_dirty =
                        (self.api.audio_set_volume)(self.player, percent as i32) != 0;
                }
                Control::Mute { muted } => {
                    (self.api.audio_set_mute)(self.player, muted as i32);
                    self.muted = muted;
                    self.audio_dirty = true;
                }
            }
        }
        Ok(self.snapshot())
    }
    fn preview(&mut self, start: f64, end: f64) -> Result<Snapshot, String> {
        let (start, end) = segment_times(start, end, self.duration)?;
        self.frozen_position = None;
        self.segment_end = Some(end);
        self.pending_seek = Some(start);
        unsafe {
            let state = (self.api.media_player_get_state)(self.player);
            if (state == 5 || state == 6) && (self.api.media_player_play)(self.player) != 0 {
                return Err("No se pudo reproducir el segmento".into());
            }
            if (self.api.media_player_is_seekable)(self.player) != 0 {
                (self.api.media_player_set_time)(self.player, start);
                self.pending_seek = None;
            }
            (self.api.media_player_set_pause)(self.player, 0);
        }
        Ok(self.snapshot())
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        // SAFETY: No callbacks point into Rust; guard/control calls cannot overlap destruction.
        unsafe {
            (self.api.media_player_stop)(self.player);
            (self.api.media_player_release)(self.player);
            if !self.media.is_null() {
                (self.api.media_release)(self.media);
            }
            (self.api.release)(self.instance);
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Control {
    Play,
    Pause,
    Seek { seconds: f64 },
    Volume { percent: u32 },
    Mute { muted: bool },
}
fn seek_time(seconds: f64, duration: i64) -> Result<i64, String> {
    if !seconds.is_finite() || duration <= 0 {
        return Err("La duración del vídeo todavía no está disponible".into());
    }
    Ok((seconds * 1000.0).clamp(0.0, duration as f64).round() as i64)
}
fn segment_times(start: f64, end: f64, duration: i64) -> Result<(i64, i64), String> {
    if !start.is_finite()
        || !end.is_finite()
        || start < 0.0
        || end <= start
        || end * 1000.0 > duration as f64 + 1.0
    {
        return Err("El segmento está fuera de la duración del vídeo".into());
    }
    let times = (seek_time(start, duration)?, seek_time(end, duration)?);
    if times.0 >= times.1 {
        return Err("El segmento debe durar al menos un milisegundo".into());
    }
    Ok(times)
}

#[tauri::command]
pub async fn vlc_load(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, PlayerState>,
    path: String,
) -> Result<Snapshot, String> {
    let path = Path::new(&path);
    if !app.asset_protocol_scope().is_allowed(path) {
        return Err("Selecciona el vídeo en el diálogo para autorizar su acceso".into());
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let metadata = File::open(&path)
        .and_then(|file| file.metadata())
        .map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("Selecciona un archivo de vídeo que no esté vacío".into());
    }
    let surface = video_surface::update(
        window,
        Bounds {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 360.0,
            visible: false,
            pixel_ratio: 1.0,
            refresh: false,
        },
    )
    .await?;
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "El motor de vídeo no está disponible")?;
    if guard.is_none() {
        *guard = Some(Player::new()?);
    }
    guard.as_mut().unwrap().load(&path, surface)
}
#[tauri::command]
pub async fn vlc_status(state: State<'_, PlayerState>, id: String) -> Result<Snapshot, String> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "El motor de vídeo no está disponible")?;
    let player = state.as_mut().ok_or("No hay un vídeo abierto")?;
    player.matches(&id)?;
    Ok(player.snapshot())
}
#[tauri::command]
pub async fn vlc_control(
    state: State<'_, PlayerState>,
    id: String,
    command: Control,
) -> Result<Snapshot, String> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "El motor de vídeo no está disponible")?;
    let player = state.as_mut().ok_or("No hay un vídeo abierto")?;
    player.matches(&id)?;
    player.control(command)
}
#[tauri::command]
pub async fn vlc_preview(
    state: State<'_, PlayerState>,
    id: String,
    start: f64,
    end: f64,
) -> Result<Snapshot, String> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "El motor de vídeo no está disponible")?;
    let player = state.as_mut().ok_or("No hay un vídeo abierto")?;
    player.matches(&id)?;
    player.preview(start, end)
}
#[tauri::command]
pub async fn vlc_surface(
    window: tauri::WebviewWindow,
    state: State<'_, PlayerState>,
    id: String,
    bounds: Bounds,
) -> Result<(), String> {
    {
        let state = state
            .0
            .lock()
            .map_err(|_| "El motor de vídeo no está disponible")?;
        state
            .as_ref()
            .ok_or("No hay un vídeo abierto")?
            .matches(&id)?;
    }
    let shared = state.0.clone();
    let expected = id.clone();
    video_surface::update_checked(window, bounds, move || {
        shared
            .lock()
            .ok()
            .is_some_and(|state| state.as_ref().is_some_and(|player| player.id == expected))
    })
    .await?;
    let mut state = state
        .0
        .lock()
        .map_err(|_| "El motor de vídeo no está disponible")?;
    if let Some(player) = state.as_mut().filter(|player| player.id == id) {
        player.surface_visible = bounds.visible;
        if bounds.refresh {
            player.poster_pending = true;
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn vlc_unload(
    window: tauri::WebviewWindow,
    state: State<'_, PlayerState>,
    id: String,
) -> Result<(), String> {
    {
        let mut state = state
            .0
            .lock()
            .map_err(|_| "El motor de vídeo no está disponible")?;
        let Some(player) = state.as_mut() else {
            return Ok(());
        };
        if player.matches(&id).is_err() {
            return Ok(());
        }
        player.clear();
    }
    let shared = state.0.clone();
    video_surface::update_checked(
        window,
        Bounds {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 360.0,
            visible: false,
            pixel_ratio: 1.0,
            refresh: false,
        },
        move || {
            shared
                .lock()
                .ok()
                .is_some_and(|state| state.as_ref().is_none_or(|player| player.id.is_empty()))
        },
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a LibVLC 3 runtime and AI_SHORTER_TEST_VIDEO fixture"]
    fn native_playback_lifecycle() {
        let path = std::env::var_os("AI_SHORTER_TEST_VIDEO").expect("AI_SHORTER_TEST_VIDEO");
        let mut player = Player::with_options(&[
            c"--vout=vdummy",
            c"--aout=adummy",
            c"--avcodec-hw=none",
            c"--dummy-chroma=I420",
        ])
        .unwrap();
        let initial = player.load(Path::new(&path), 0).unwrap();
        player.surface_visible = true;
        let wait = |player: &mut Player, predicate: fn(&Snapshot) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            loop {
                player.tick();
                let snapshot = player.snapshot();
                if predicate(&snapshot) {
                    return snapshot;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "timeout in state {} at {}",
                    snapshot.state,
                    snapshot.position
                );
                assert_ne!(snapshot.state, "error");
                std::thread::sleep(Duration::from_millis(25));
            }
        };
        let loaded = wait(&mut player, |s| {
            s.duration.is_some() && s.seekable && s.state == "paused"
        });
        assert!(loaded.duration.unwrap() >= 3.0);
        assert!(!loaded.playing, "selection must not autoplay audio");
        assert!(player.matches("stale-session").is_err());
        player.control(Control::Play).unwrap();
        wait(&mut player, |s| {
            s.playing && s.position > 0.25 && s.decoded_frames > 0 && s.displayed_frames > 0
        });
        player.control(Control::Pause).unwrap();
        wait(&mut player, |s| s.state == "paused");
        assert_eq!(
            player
                .control(Control::Seek { seconds: 3.0 })
                .unwrap()
                .position,
            3.0
        );
        assert_eq!(
            player
                .control(Control::Volume { percent: 25 })
                .unwrap()
                .volume,
            25
        );
        assert!(player.control(Control::Mute { muted: true }).unwrap().muted);
        player.preview(1.0, 2.0).unwrap();
        let stopped = wait(&mut player, |s| {
            !s.segment_active && s.position == 2.0 && !s.playing
        });
        assert_eq!(stopped.position, 2.0);
        let replaced = player.load(Path::new(&path), 0).unwrap();
        wait(&mut player, |s| s.state == "paused" && s.duration.is_some());
        assert_ne!(initial.id, replaced.id);
        assert!(player.matches(&initial.id).is_err());
        player.clear();
        assert!(player.matches(&replaced.id).is_err());
        assert!(player.media.is_null());
    }

    #[test]
    fn validates_seek_and_segment_boundaries() {
        assert_eq!(seek_time(-5.0, 60_000), Ok(0));
        assert_eq!(seek_time(65.0, 60_000), Ok(60_000));
        assert_eq!(seek_time(1.234, 60_000), Ok(1234));
        assert!(seek_time(f64::NAN, 60_000).is_err());
        assert!(seek_time(1.0, 0).is_err());
        assert_eq!(segment_times(5.0, 35.0, 60_000), Ok((5000, 35000)));
        for (start, end) in [
            (-1.0, 1.0),
            (5.0, 5.0),
            (5.0, 61.0),
            (f64::NAN, 10.0),
            (0.0, f64::INFINITY),
        ] {
            assert!(segment_times(start, end, 60_000).is_err());
        }
    }
}
