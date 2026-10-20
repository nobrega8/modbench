//! The command-line tool's informational options and argument errors.

use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_modbench-cli"))
        .args(args)
        .output()
        .expect("modbench-cli runs")
}

#[test]
fn help_lists_the_commands() {
    for flag in ["-h", "--help"] {
        let out = cli(&[flag]);
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout);
        // "modbench-cli.exe" on Windows
        assert!(text.contains("Usage: modbench-cli"), "{text}");
        for command in ["ports", "read", "map", "check", "scan", "validate"] {
            assert!(text.contains(command), "{command} missing from {text}");
        }
    }
}

#[test]
fn version_prints_the_package_version() {
    for flag in ["-V", "--version"] {
        let out = cli(&[flag]);
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            concat!("modbench-cli ", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn invalid_arguments_fail_with_a_message() {
    for args in [&["--bogus"][..], &["read"], &["frobnicate"]] {
        let out = cli(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).starts_with("error:"),
            "{args:?}"
        );
    }
}
