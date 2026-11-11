// Release builds run without a console window. Ignored on non-Windows targets.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    reposhelf_lib::run()
}
