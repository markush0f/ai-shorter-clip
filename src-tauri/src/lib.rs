mod player;
mod runtime;
mod video_surface;
mod vlc_api;

pub use runtime::prepare_process;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let player = player::PlayerState::default();
            player.start_guard();
            app.manage(player);
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) {
                // Stop LibVLC before the OS destroys its drawing window.
                window.state::<player::PlayerState>().shutdown();
            }
        })
        .invoke_handler(tauri::generate_handler![
            player::vlc_load,
            player::vlc_status,
            player::vlc_control,
            player::vlc_preview,
            player::vlc_surface,
            player::vlc_unload,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
