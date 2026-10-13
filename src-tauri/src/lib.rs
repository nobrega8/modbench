pub mod codec;
mod commands;
pub mod mbmap;
pub mod modbus;
pub mod planner;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
