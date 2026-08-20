#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use escpos::driver::Driver;

#[path = "../src/models/mod.rs"]
mod models;
#[path = "../src/printer/mod.rs"]
mod printer;

use models::{Align, Block, BarcodeSymbology, Column, ImageMime, PrintDocument, PrinterTarget};
use printer::GraphicsBackend;

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
    let lines = printer::text::wrap_text("one two three four", 9);

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
    assert!(
        bytes.windows(48).any(|value| value
            == b"Coffee                           2          4.50"),
        "column row must fill the character line with spaces, got {bytes:?}"
    );
    assert!(
        !bytes.windows(2).any(|value| value == [0x1B, b'$']),
        "emulator-incompatible ESC $ must not be sent, got {bytes:?}"
    );
    assert!(
        !bytes.windows(2).any(|value| value == [0x1D, b'W']),
        "emulator-incompatible GS W must not be sent, got {bytes:?}"
    );
}

#[test]
fn decodes_data_url_images() {
    let bytes = printer::render::decode_image("data:image/png;base64,aGVsbG8=").unwrap();

    assert_eq!(bytes, b"hello");
}

#[test]
fn raster_image_keeps_following_text_in_the_byte_stream() {
    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render::render_with_graphics(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![
                Block::Image {
                    data: ONE_PX_PNG.into(),
                    mime: ImageMime::Png,
                    max_width_dots: Some(8),
                    align: Some(Align::Center),
                },
                Block::Text {
                    value: "HEADER".into(),
                    style: None,
                },
            ],
        },
        GraphicsBackend::Emulator,
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    let after_raster = skip_gs_v0(&bytes);
    assert_eq!(
        after_raster.first().copied(),
        Some(0x0A),
        "GS v 0 must be followed by LF so emulators resume text, got {bytes:?}"
    );
    assert!(
        after_raster.windows(6).any(|value| value == b"HEADER"),
        "text after the logo must stay after GS v 0, got {bytes:?}"
    );
}

#[test]
fn ean13_barcode_is_raster_and_keeps_following_footer_text() {
    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render::render_with_graphics(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![
                Block::Barcode {
                    value: "4959920317636".into(),
                    symbology: BarcodeSymbology::Ean13,
                    align: Some(Align::Center),
                    print_value: Some(true),
                },
                Block::Text {
                    value: "Footer sample".into(),
                    style: None,
                },
            ],
        },
        GraphicsBackend::Emulator,
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    assert!(
        !bytes.windows(2).any(|value| value == [0x1D, b'k']),
        "EAN-13 must not use GS k, got {bytes:?}"
    );
    let after_raster = skip_gs_v0(&bytes);
    assert_eq!(
        after_raster.first().copied(),
        Some(0x0A),
        "barcode raster must be followed by LF so emulators resume text, got {bytes:?}"
    );
    assert!(
        after_raster.windows(13).any(|value| value == b"Footer sample"),
        "footer must stay after the barcode raster, got {bytes:?}"
    );
    assert!(
        after_raster.windows(13).any(|value| value == b"4959920317636"),
        "barcode digits must print under the raster, got {bytes:?}"
    );
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

#[test]
fn encodes_ascii_hyphen_and_divider_as_0x2d() {
    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render_document(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![
                Block::Text {
                    value: "ORD-8".into(),
                    style: None,
                },
                Block::Divider { character: None },
            ],
        },
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    assert!(
        bytes.windows(5).any(|value| value == b"ORD-8"),
        "hyphen must stay ASCII 0x2D, got {bytes:?}"
    );
    assert!(
        bytes.windows(8).any(|value| value == b"--------"),
        "divider must repeat ASCII '-', got {bytes:?}"
    );
}

#[test]
fn encodes_pound_yen_and_cent_as_pc858_bytes() {
    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render_document(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![Block::Text {
                value: "£1 ¥2 ¢3".into(),
                style: None,
            }],
        },
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    let expected = [0x9C, b'1', b' ', 0xBE, b'2', b' ', 0xBD, b'3'];
    assert!(
        bytes.windows(expected.len()).any(|value| value == expected),
        "£ ¥ ¢ must be PC858 0x9C/0xBE/0xBD, got {bytes:?}"
    );
}

#[test]
fn encodes_euro_as_pc858_byte() {
    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render_document(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![Block::Text {
                value: "€2.50".into(),
                style: None,
            }],
        },
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    assert!(
        bytes.windows(5).any(|value| value == [0xD5, b'2', b'.', b'5', b'0']),
        "euro must be PC858 0xD5, got {bytes:?}"
    );
}

#[test]
fn parses_host_and_port_for_network_printers() {
    assert_eq!(
        printer::parse_host_port("127.0.0.1:9100"),
        Some(("127.0.0.1".into(), 9100))
    );
    assert_eq!(printer::parse_host_port("127.0.0.1"), None);
    assert_eq!(printer::parse_host_port(":9100"), None);
}

#[cfg(debug_assertions)]
#[test]
fn lists_local_network_emulator_in_debug_builds() {
    let printers = printer::list_printers().expect("listing printers should not fail");

    assert!(printers.iter().any(|printer| printer.path == "127.0.0.1:9100"));
}

#[test]
fn hardware_ean13_uses_escpos_gs_k() {
    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render_document(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![Block::Barcode {
                value: "4959920317636".into(),
                symbology: BarcodeSymbology::Ean13,
                align: Some(Align::Center),
                print_value: Some(true),
            }],
        },
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    assert!(
        bytes.windows(2).any(|value| value == [0x1D, b'k']),
        "USB/network printers must use escpos-rs GS k, got {bytes:?}"
    );
}

#[test]
fn hardware_image_uses_escpos_bit_image() {
    let driver = RecordingDriver::default();
    let bytes = driver.0.clone();
    printer::render_document(
        driver,
        &PrintDocument {
            paper_width_mm: None,
            character_set: None,
            blocks: vec![Block::Image {
                data: ONE_PX_PNG.into(),
                mime: ImageMime::Png,
                max_width_dots: Some(8),
                align: Some(Align::Center),
            }],
        },
    )
    .unwrap();

    let bytes = bytes.lock().unwrap();
    assert!(
        bytes.windows(3).any(|value| value == [0x1D, b'v', b'0']),
        "hardware images must use escpos-rs GS v 0 bit image, got {bytes:?}"
    );
}

#[test]
fn local_emulator_uses_dev_raster_backend() {
    assert_eq!(
        printer::graphics_backend_for(&PrinterTarget::Network {
            host: "127.0.0.1".into(),
            port: 9100,
        }),
        GraphicsBackend::Emulator
    );
    assert_eq!(
        printer::graphics_backend_for(&PrinterTarget::Network {
            host: "192.168.1.50".into(),
            port: 9100,
        }),
        GraphicsBackend::Escpos
    );
    assert_eq!(
        printer::graphics_backend_for(&PrinterTarget::WindowsUsbByPath {
            path: r"\\?\usb#vid_1234".into(),
        }),
        GraphicsBackend::Escpos
    );
}

const ONE_PX_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

fn skip_gs_v0(bytes: &[u8]) -> &[u8] {
    let prefix = [0x1D, b'v', b'0'];
    let start = bytes.windows(prefix.len()).position(|window| window == prefix).unwrap();
    let header = &bytes[start + 3..];
    let width = u16::from(header[1]) | (u16::from(header[2]) << 8);
    let height = u16::from(header[3]) | (u16::from(header[4]) << 8);
    let length = 5 + (width as usize * height as usize);
    &header[length..]
}
