use tauri::{Emitter, Manager};

pub mod ascii;
pub mod codec;
mod commands;
pub mod firmware;
pub mod generators;
pub mod library;
pub mod mbmap;
mod mbmap_v2;
pub mod modbus;
pub mod online;
pub mod planner;
pub mod poller;
pub mod serve;
pub mod simulator;
pub mod tap;
pub mod udp;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first: a second launch (e.g. double-clicking a
        // .mbmap) hands its file to the running window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = argv.iter().skip(1).find(|a| commands::is_openable_file(a)) {
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
        .plugin(tauri_plugin_clipboard_manager::init())
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
            commands::save_bytes,
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
            commands::check_functions,
            commands::scan_addresses,
            commands::sim_start,
            commands::sim_stop,
            commands::sim_add_device,
            commands::sim_remove_device,
            commands::sim_set_unit,
            commands::sim_set_generators,
            commands::sim_info,
            commands::sim_open_window,
            commands::sim_close_window,
            commands::sim_values,
            commands::sim_set_value,
            commands::sim_load,
            commands::sim_read,
            commands::sim_write,
        ])
        .on_window_event(|window, event| {
            if !matches!(event, tauri::WindowEvent::Destroyed) {
                return;
            }
            let app = window.app_handle();
            match window.label() {
                // The main window shows the simulator again.
                commands::SIM_WINDOW => {
                    let _ = app.emit("sim-window", false);
                }
                // The simulator's window does not outlive the app's.
                "main" => {
                    if let Some(sim) = app.get_webview_window(commands::SIM_WINDOW) {
                        let _ = sim.close();
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
