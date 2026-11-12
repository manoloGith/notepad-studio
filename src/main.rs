// Notepad Studio — lightweight cross-platform text editor.

// On Windows, the default Rust binary uses the "console" subsystem, so Windows
// spawns a terminal window alongside the GUI and stdout (e.g. the plugin
// scanner's logs) prints into it. Build release binaries with the "windows"
// (GUI) subsystem so no console appears. Debug builds keep the console so
// `println!`/`eprintln!` diagnostics stay visible during development. No-op on
// non-Windows targets.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    notepad_studio::main()
}
