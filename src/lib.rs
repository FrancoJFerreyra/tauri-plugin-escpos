#[cfg(feature = "tauri-plugin")]
mod commands;
mod models;
#[cfg(feature = "tauri-plugin")]
mod printer;
#[cfg(feature = "tauri-plugin")]
#[path = "virtualPrinter/mod.rs"]
mod virtual_printer;

#[cfg(feature = "tauri-plugin")]
use tauri::{
    plugin::{Builder, TauriPlugin},
    Wry,
};

#[cfg(feature = "tauri-plugin")]
pub use commands::*;
pub use models::*;

#[cfg(feature = "tauri-plugin")]
pub fn init() -> TauriPlugin<Wry> {
    Builder::new("escpos")
        .invoke_handler(tauri::generate_handler![
            commands::print,
            commands::list_printers,
            commands::test_printer,
            commands::self_test
        ])
        .build()
}