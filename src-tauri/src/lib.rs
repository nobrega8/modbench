pub mod codec;
mod commands;
pub mod mbmap;
pub mod modbus;
pub mod planner;
pub mod poller;
pub mod tap;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::list_serial_ports,
            commands::connect,
            commands::disconnect,
            commands::read_raw,
            commands::write_registers,
            commands::write_coils,
            commands::read_value,
            commands::write_value,
            commands::open_map,
            commands::save_map,
            commands::start_polling,
            commands::stop_polling,
            commands::poll_map_once,
            commands::write_map_value,
            commands::save_text,
            commands::startup_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
