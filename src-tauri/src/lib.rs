use tauri::{Emitter, Manager};

pub mod ascii;
pub mod codec;
mod commands;
pub mod firmware;
pub mod generators;
pub mod library;
pub mod mapfind;
pub mod mbmap;
mod mbmap_v2;
pub mod modbus;
pub mod netscan;
pub mod online;
pub mod planner;
pub mod poller;
pub mod serve;
pub mod simulator;
pub mod tap;
pub mod udp;

const USAGE: &str = "\
Modbus client and simulator, with device maps (.mbmap)

Usage: modbench [FILE]

Arguments:
  [FILE]  A device map (.mbmap.json, .mbmap) or a simulator setup
          (.mbsim) to open

Options:
  -h, --help     Print help
  -V, --version  Print version

Command-line access without the window: modbench-cli --help
";

/// What to print, and the exit code, when the arguments ask for help or
/// the version, or hold an unknown option; `None` opens the window. Other
/// arguments (a file to open, or what the system adds, such as macOS's
/// `-psn_…`) are left to the app.
pub fn command_line(args: &[String]) -> Option<(String, i32)> {
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => return Some((USAGE.to_string(), 0)),
            "-V" | "--version" => {
                return Some((format!("modbench {}\n", env!("CARGO_PKG_VERSION")), 0));
            }
            "--" => break,
            a if a.starts_with("--") => {
                return Some((
                    format!(
                        "error: unexpected argument '{a}'\n\nUsage: modbench [FILE]\n\n\
                         For more information, try '--help'.\n"
                    ),
                    2,
                ));
            }
            _ => {}
        }
    }
    None
}

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
            commands::set_single_write_fc16,
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
            commands::scan_network,
            commands::local_interfaces,
            commands::demo_map,
            commands::find_maps,
            commands::send_pdu,
            commands::send_raw,
            commands::startup_file,
            commands::library_folder,
            commands::library_list,
            commands::library_import,
            commands::library_delete,
            commands::library_open_folder,
            commands::library_edit,
            commands::open_link,
            commands::library_online_index,
            commands::library_online_import,
            commands::library_online_map,
            commands::library_duplicate,
            commands::vendor_icon,
            commands::read_device_id,
            commands::check_functions,
            commands::largest_read,
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
            commands::builder_open_window,
            commands::builder_close_window,
            commands::builder_take_target,
            commands::validate_map,
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
                // The tool windows do not outlive the app's.
                "main" => {
                    for label in [commands::SIM_WINDOW, commands::BUILDER_WINDOW] {
                        if let Some(window) = app.get_webview_window(label) {
                            let _ = window.close();
                        }
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Stop polling and free the serial port or socket on the way out.
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<commands::AppState>();
                tauri::async_runtime::block_on(state.shut_down());
            }
        });
}

#[cfg(test)]
mod tests {
    use super::command_line;

    fn args(list: &[&str]) -> Vec<String> {
        std::iter::once("modbench")
            .chain(list.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn help_and_version_answer_without_the_window() {
        for flag in ["-h", "--help"] {
            let (text, code) = command_line(&args(&[flag])).unwrap();
            assert_eq!(code, 0);
            assert!(
                text.starts_with("Modbus client and simulator") && text.contains("Usage: modbench")
            );
        }
        for flag in ["-V", "--version"] {
            let (text, code) = command_line(&args(&["map.mbmap.json", flag])).unwrap();
            assert_eq!(
                (text.trim(), code),
                (concat!("modbench ", env!("CARGO_PKG_VERSION")), 0)
            );
        }
    }

    #[test]
    fn files_and_system_arguments_open_the_window() {
        assert_eq!(command_line(&args(&[])), None);
        assert_eq!(command_line(&args(&["map.mbmap.json"])), None);
        assert_eq!(command_line(&args(&["-psn_0_12345"])), None);
        assert_eq!(command_line(&args(&["--", "--help"])), None);
    }

    #[test]
    fn unknown_options_are_an_error() {
        let (text, code) = command_line(&args(&["--bogus"])).unwrap();
        assert_eq!(code, 2);
        assert!(text.contains("unexpected argument '--bogus'"));
    }
}
