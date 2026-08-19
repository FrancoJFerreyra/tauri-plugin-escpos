use tauri::command;

use crate::models::{EscposError, PrintDocument, PrinterInfo, PrinterTarget};
use crate::printer;

#[command]
pub fn print(printer: PrinterTarget, document: PrintDocument) -> Result<(), EscposError> {
    crate::printer::print_document(&printer, &document)
}

#[command]
pub fn list_printers() -> Result<Vec<PrinterInfo>, EscposError> {
    printer::list_printers()
}

#[command]
pub fn test_printer(printer: PrinterTarget) -> Result<(), EscposError> {
    crate::printer::test_printer(&printer)
}

#[command]
pub fn self_test() -> Result<Vec<PrinterInfo>, EscposError> {
    printer::self_test()
}