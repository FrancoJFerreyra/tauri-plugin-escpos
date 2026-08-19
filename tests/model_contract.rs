use tauri_plugin_escpos::{
    BarcodeSymbology, Block, CharacterSet, ErrorCode, PaperWidth, PrintDocument, PrinterTarget,
};

#[test]
fn document_defaults_to_80mm_and_pc858() {
    let document = PrintDocument {
        paper_width_mm: None,
        character_set: None,
        blocks: vec![],
    };

    assert_eq!(document.paper_width_mm(), PaperWidth::Mm80);
    assert_eq!(document.character_set(), CharacterSet::Pc858);
    assert_eq!(document.characters_per_line(), 48);
}

#[test]
fn rejects_columns_without_content() {
    let block = Block::Columns { columns: vec![] };

    let error = block.validate().unwrap_err();

    assert_eq!(error.code, ErrorCode::InvalidDocument);
    assert_eq!(error.message, "Columns block must contain at least one column");
}

#[test]
fn rejects_invalid_ean13_data() {
    let block = Block::Barcode {
        value: "not-an-ean".into(),
        symbology: BarcodeSymbology::Ean13,
        align: None,
        print_value: None,
    };

    let error = block.validate().unwrap_err();

    assert_eq!(error.code, ErrorCode::InvalidDocument);
    assert_eq!(error.message, "EAN-13 barcode must contain 12 or 13 digits");
}

#[test]
fn serializes_public_types_with_camel_case_fields() {
    let target = PrinterTarget::WindowsUsbByVidPid {
        vendor_id: 0x04b8,
        product_id: 0x0202,
    };

    let value = serde_json::to_value(target).unwrap();

    assert_eq!(value["kind"], "windows_usb");
    assert_eq!(value["vendorId"], 0x04b8);
    assert_eq!(value["productId"], 0x0202);
}

#[test]
fn serializes_network_printer_target() {
    let target = PrinterTarget::Network {
        host: "127.0.0.1".into(),
        port: 9100,
    };

    let value = serde_json::to_value(&target).unwrap();

    assert_eq!(value["kind"], "network");
    assert_eq!(value["host"], "127.0.0.1");
    assert_eq!(value["port"], 9100);
}

#[test]
fn deserializes_network_printer_target() {
    let target: PrinterTarget = serde_json::from_value(serde_json::json!({
        "kind": "network",
        "host": "127.0.0.1",
        "port": 9100
    }))
    .unwrap();

    assert_eq!(
        target,
        PrinterTarget::Network {
            host: "127.0.0.1".into(),
            port: 9100,
        }
    );
}
