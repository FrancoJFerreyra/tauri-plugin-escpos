const COMMANDS: &[&str] = &["print", "list_printers", "test_printer"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}