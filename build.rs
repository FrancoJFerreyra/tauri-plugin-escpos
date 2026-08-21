const COMMANDS: &[&str] = &["print", "list_printers", "test_printer", "self_test"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}