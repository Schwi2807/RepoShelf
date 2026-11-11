//! RepoShelf Tauri shell. Commands and app state arrive in M4; the window `main` is defined in
//! `tauri.conf.json` for now (M4.3 moves it into code for `data_directory`).

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running RepoShelf");
}
