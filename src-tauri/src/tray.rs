use serde::Serialize;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, Runtime, Window, WindowEvent};

use crate::settings::SettingsStore;

const MAIN_WINDOW_LABEL: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_DOWNLOADS: &str = "downloads";
const MENU_SETTINGS: &str = "settings";
const MENU_QUIT: &str = "quit";

#[derive(Clone, Serialize)]
struct TrayNavigatePayload {
    page: &'static str,
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut tray = TrayIconBuilder::with_id("modelhub-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("ModelHub Windows")
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_icon_event);

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

pub fn handle_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        if should_minimize_to_tray(window) {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}

fn should_minimize_to_tray<R: Runtime>(window: &Window<R>) -> bool {
    SettingsStore::for_manager(window)
        .map(|store| store.minimize_to_tray())
        .unwrap_or(true)
}

fn build_menu<R: Runtime, M: Manager<R>>(manager: &M) -> tauri::Result<Menu<R>> {
    let open = MenuItem::with_id(manager, MENU_OPEN, "Open ModelHub", true, None::<&str>)?;
    let downloads = MenuItem::with_id(
        manager,
        MENU_DOWNLOADS,
        "Downloads: 0 active",
        true,
        None::<&str>,
    )?;
    let ollama = MenuItem::with_id(
        manager,
        "ollama-status",
        "Ollama: Not checked",
        false,
        None::<&str>,
    )?;
    let lm_studio = MenuItem::with_id(
        manager,
        "lm-studio-status",
        "LM Studio: Not checked",
        false,
        None::<&str>,
    )?;
    let pause_all = MenuItem::with_id(
        manager,
        "pause-all-downloads",
        "Pause all downloads",
        false,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(manager, MENU_SETTINGS, "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(manager, MENU_QUIT, "Quit", true, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(manager)?;
    let separator_two = PredefinedMenuItem::separator(manager)?;

    Menu::with_items(
        manager,
        &[
            &open,
            &separator_one,
            &downloads,
            &ollama,
            &lm_studio,
            &pause_all,
            &separator_two,
            &settings,
            &quit,
        ],
    )
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_OPEN => show_main_window(app),
        MENU_DOWNLOADS => open_page(app, "downloads"),
        MENU_SETTINGS => open_page(app, "settings"),
        MENU_QUIT => app.exit(0),
        _ => {}
    }
}

fn handle_tray_icon_event<R: Runtime>(tray: &tauri::tray::TrayIcon<R>, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        show_main_window(tray.app_handle());
    }
}

fn open_page<R: Runtime>(app: &AppHandle<R>, page: &'static str) {
    show_main_window(app);
    let _ = app.emit("tray:navigate", TrayNavigatePayload { page });
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
