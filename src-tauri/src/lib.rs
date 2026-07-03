use tauri::Manager;

mod commands;
mod downloads;
mod errors;
mod hf;
mod model_actions;
mod models;
mod paths;
mod runtimes;
mod scanner;
mod settings;
mod system_info;
mod tray;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::get_resolved_paths,
            commands::scan_models,
            commands::get_ollama_runtime_status,
            commands::get_system_info,
            commands::search_hf_models,
            commands::get_hf_model_details,
            commands::start_download,
            commands::list_downloads,
            commands::cancel_download,
            commands::pause_download,
            commands::resume_download,
            commands::install_download,
            commands::open_path,
            commands::delete_model,
        ])
        .setup(|app| {
            let download_manager = downloads::DownloadManager::for_app(app)
                .map_err(|error| tauri::Error::Anyhow(anyhow::anyhow!(error)))?;
            app.manage(download_manager);
            tray::setup(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            tray::handle_window_event(window, event);
        })
        .run(tauri::generate_context!())
}
