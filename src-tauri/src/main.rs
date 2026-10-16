// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some((text, code)) = modbench_lib::command_line(&args) {
        if code == 0 {
            print!("{text}");
        } else {
            eprint!("{text}");
        }
        std::process::exit(code);
    }
    modbench_lib::run()
}
