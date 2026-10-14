use tauri::{Emitter, Manager};

pub mod ascii;
pub mod codec;
mod commands;
pub mod firmware;
pub mod library;
pub mod mbmap;
mod mbmap_v2;
pub mod modbus;
pub mod online;
pub mod planner;
pub mod poller;
pub mod simulator;
pub mod tap;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first: a second launch (e.g. double-clicking a
        // .mbmap) hands its file to the running window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = argv.iter().skip(1).find(|a| mbmap::is_map_file(a)) {
                let _ = app.emit("open-file", path);
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::installed_with_apt,
            commands::list_serial_ports,
            commands::connect,
            commands::disconnect,
            commands::set_read_only,
            commands::link_lost,
            commands::reconnect,
            commands::read_raw,
            commands::write_registers,
            commands::write_coils,
            commands::read_value,
            commands::write_value,
            commands::open_map,
            commands::save_map,
            commands::convert_map,
            commands::start_polling,
            commands::stop_polling,
            commands::poll_map_once,
            commands::write_map_value,
            commands::save_text,
            commands::append_text,
            commands::read_text,
            commands::scan_units,
            commands::scan_serial,
            commands::cancel_scan,
            commands::startup_file,
            commands::library_folder,
            commands::library_list,
            commands::library_import,
            commands::library_delete,
            commands::library_open_folder,
            commands::open_link,
            commands::library_online_index,
            commands::library_online_import,
            commands::vendor_icon,
            commands::read_device_id,
            commands::scan_addresses,
            commands::sim_start,
            commands::sim_stop,
            commands::sim_status,
            commands::sim_load,
            commands::sim_read,
            commands::sim_write,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
