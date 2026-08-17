#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use escpos::driver::Driver;

#[path = "../src/models.rs"]
mod models;
#[path = "../src/printer.rs"]
mod printer;

use models::{Align, Block, Column, ErrorCode, PrintDocument};

#[derive(Clone, Default)]
struct RecordingDriver(Arc<Mutex<Vec<u8>>>);

impl Driver for RecordingDriver {
    fn name(&self) -> String {
        "recording".into()
    }

    fn write(&self, data: &[u8]) -> escpos::errors::Result<()> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(())
    }

    fn read(&self, _buffer: &mut [u8]) -> escpos::errors::Result<usize> {
        Ok(0)
    }

    fn flush(&self) -> escpos::errors::Result<()> {
        Ok(())
    }
}

#[test]
fn wraps_text_to_paper_width_without_losing_words() {
    let lines = printer::wrap_text("one two three four", 9);

    assert_eq!(lines, vec!["one two", "three", "four"]);
}

#[test]
fn formats_columns_using_remaining_width_for_first_column() {
    let columns = vec![
        Column {
            text: "Coffee".into(),
            width: None,
            align: None,
            style: None,
        },
        Column {
            text: "2".into(),
            width: Some(0.2),
            align: Some(Align::Right),
            style: None,
        },
        Column {
            text: "4.50".into(),
            width: Some(0.3),
            align: Some(Align::Right),
            style: None,
        },
    ];

    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render_document(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![Block::Columns { columns }],
        },
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    assert!(bytes.windows(24).any(|value| value == b"Coffee                  "));
    assert!(bytes.windows(10).any(|value| value == b"         2"));
    assert!(bytes.windows(14).any(|value| value == b"          4.50"));
}

#[test]
fn decodes_data_url_images() {
    let bytes = printer::decode_image("data:image/png;base64,aGVsbG8=").unwrap();

    assert_eq!(bytes, b"hello");
}

#[test]
fn sends_cut_only_when_document_contains_cut_block() {
    let without_cut_driver = RecordingDriver::default();
    let without_cut_bytes = without_cut_driver.0.clone();
    printer::render_document(
        without_cut_driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![Block::Text {
                value: "Hello".into(),
                style: None,
            }],
        },
    )
    .unwrap();

    let with_cut_driver = RecordingDriver::default();
    let with_cut_bytes = with_cut_driver.0.clone();
    printer::render_document(
        with_cut_driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![Block::Cut {
                partial: Some(false),
            }],
        },
    )
    .unwrap();

    let full_cut = [0x1d, b'V', b'A', 0];
    assert!(!without_cut_bytes
        .lock()
        .unwrap()
        .windows(full_cut.len())
        .any(|bytes| bytes == full_cut));
    assert!(with_cut_bytes
        .lock()
        .unwrap()
        .windows(full_cut.len())
        .any(|bytes| bytes == full_cut));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn reports_unsupported_platform_outside_windows() {
    let error = printer::list_printers().unwrap_err();

    assert_eq!(error.code, ErrorCode::UnsupportedPlatform);
    assert_eq!(
        error.message,
        "Windows USB printing is only available on Windows"
    );
}
