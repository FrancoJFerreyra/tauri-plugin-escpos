use base64::{engine::general_purpose::STANDARD, Engine as _};
use escpos::{
    driver::Driver,
    printer::Printer,
    utils::{
        BarcodeFont, BarcodeHeight, BarcodeOption, BarcodePosition, BarcodeWidth, BitImageOption,
        BitImageSize, JustifyMode, PageCode, Protocol, QRCodeCorrectionLevel, QRCodeModel,
        QRCodeOption, UnderlineMode,
    },
};

use crate::models::{
    Align, BarcodeSymbology, Block, CharacterSet, Column, ErrorCode, EscposError, FontSize,
    PrintDocument, PrinterInfo, PrinterTarget, TextStyle,
};

#[cfg(target_os = "windows")]
use crate::models::PrinterBackend;
#[cfg(target_os = "windows")]
use escpos::driver::WindowsUsbPrintDriver;

#[cfg(target_os = "windows")]
pub fn print_document(
    target: &PrinterTarget,
    document: &PrintDocument,
) -> Result<(), EscposError> {
    match target {
        PrinterTarget::WindowsUsbByPath { path } => {
            let driver = WindowsUsbPrintDriver::open(path).map_err(|error| {
                EscposError::new(
                    ErrorCode::OpenFailed,
                    format!("Failed to open Windows USB printer: {error}"),
                )
            })?;
            render_document(driver, document)
        }
        PrinterTarget::WindowsUsbByVidPid {
            vendor_id,
            product_id,
        } => {
            let driver = WindowsUsbPrintDriver::open_by_vid_pid(*vendor_id, *product_id)
                .map_err(|error| {
                    EscposError::new(
                        ErrorCode::PrinterNotFound,
                        format!(
                            "Windows USB printer {:04x}:{:04x} was not found: {error}",
                            vendor_id, product_id
                        ),
                    )
                })?;
            render_document(driver, document)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn print_document(
    _target: &PrinterTarget,
    _document: &PrintDocument,
) -> Result<(), EscposError> {
    Err(unsupported_platform())
}

#[cfg(target_os = "windows")]
pub fn list_printers() -> Result<Vec<PrinterInfo>, EscposError> {
    WindowsUsbPrintDriver::list()
        .map_err(|error| {
            EscposError::new(
                ErrorCode::OpenFailed,
                format!("Failed to list Windows USB printers: {error}"),
            )
        })
        .map(|printers| {
            printers
                .into_iter()
                .map(|printer| PrinterInfo {
                    id: printer.device_path.clone(),
                    path: printer.device_path,
                    name: None,
                    vendor_id: printer.vendor_id,
                    product_id: printer.product_id,
                    backend: PrinterBackend::WindowsUsb,
                })
                .collect()
        })
}

#[cfg(not(target_os = "windows"))]
pub fn list_printers() -> Result<Vec<PrinterInfo>, EscposError> {
    Err(unsupported_platform())
}

pub fn test_printer(target: &PrinterTarget) -> Result<(), EscposError> {
    let document = PrintDocument {
        paper_width_mm: None,
        character_set: None,
        blocks: vec![
            Block::Text {
                value: "ESC/POS PRINTER TEST".into(),
                style: Some(TextStyle {
                    bold: Some(true),
                    align: Some(Align::Center),
                    ..TextStyle::default()
                }),
            },
            Block::Divider {
                character: Some("=".into()),
            },
            Block::Columns {
                columns: vec![
                    Column {
                        text: "Character set".into(),
                        width: None,
                        align: None,
                        style: None,
                    },
                    Column {
                        text: "PC858".into(),
                        width: Some(0.3),
                        align: Some(Align::Right),
                        style: None,
                    },
                ],
            },
            Block::Barcode {
                value: "123456789012".into(),
                symbology: BarcodeSymbology::Ean13,
                align: Some(Align::Center),
                print_value: Some(true),
            },
            Block::Feed { lines: Some(2) },
            Block::Cut {
                partial: Some(false),
            },
        ],
    };
    print_document(target, &document)
}

#[cfg(not(target_os = "windows"))]
fn unsupported_platform() -> EscposError {
    EscposError::new(
        ErrorCode::UnsupportedPlatform,
        "Windows USB printing is only available on Windows",
    )
}

pub fn render_document<D: Driver>(
    driver: D,
    document: &PrintDocument,
) -> Result<(), EscposError> {
    document.validate()?;

    let mut printer = Printer::new(driver, Protocol::default(), None);
    let page_code = match document.character_set() {
        CharacterSet::Pc858 => PageCode::PC858,
    };
    printer
        .init()
        .and_then(|printer| printer.page_code(page_code))
        .map_err(print_error)?;

    let line_width = document.characters_per_line();
    for block in &document.blocks {
        render_block(&mut printer, block, line_width)?;
    }

    printer.print().map_err(print_error)?;
    Ok(())
}

fn render_block<D: Driver>(
    printer: &mut Printer<D>,
    block: &Block,
    line_width: usize,
) -> Result<(), EscposError> {
    match block {
        Block::Text { value, style } => {
            apply_style(printer, style.as_ref())?;
            let width = line_width / horizontal_scale(style.as_ref());
            for line in wrap_text(value, width) {
                writeln_encoded(printer, &line)?;
            }
        }
        Block::Feed { lines } => {
            printer.feeds(lines.unwrap_or(1)).map_err(print_error)?;
        }
        Block::Divider { character } => {
            reset_style(printer)?;
            let value = character.as_deref().unwrap_or("-");
            writeln_encoded(printer, &value.repeat(line_width))?;
        }
        Block::Columns { columns } => {
            render_columns(printer, columns, line_width)?;
        }
        Block::Image {
            data,
            max_width_dots,
            align,
            ..
        } => {
            set_alignment(printer, align.unwrap_or(Align::Center))?;
            let bytes = decode_image(data)?;
            let max_width = max_width_dots
                .map(|width| u32::from(width.max(8) / 8 * 8))
                .or(Some(512));
            let option = BitImageOption::new(max_width, None, BitImageSize::Normal)
                .map_err(invalid_document_error)?;
            printer
                .bit_image_from_bytes_option(&bytes, option)
                .map_err(invalid_document_error)?;
        }
        Block::Barcode {
            value,
            symbology,
            align,
            print_value,
        } => {
            set_alignment(printer, align.unwrap_or(Align::Center))?;
            let position = if print_value.unwrap_or(true) {
                BarcodePosition::Below
            } else {
                BarcodePosition::None
            };
            let option = BarcodeOption::new(
                BarcodeWidth::M,
                BarcodeHeight::S,
                BarcodeFont::A,
                position,
            );
            match symbology {
                BarcodeSymbology::Ean13 => printer.ean13_option(value, option),
                BarcodeSymbology::Ean8 => printer.ean8_option(value, option),
                BarcodeSymbology::Upca => printer.upca_option(value, option),
                BarcodeSymbology::Code39 => printer.code39_option(value, option),
            }
            .map_err(invalid_document_error)?;
        }
        Block::Qr { value, size, align } => {
            set_alignment(printer, align.unwrap_or(Align::Center))?;
            let option = QRCodeOption::new(
                QRCodeModel::Model2,
                size.unwrap_or(4),
                QRCodeCorrectionLevel::M,
            );
            printer
                .qrcode_option(value, option)
                .map_err(invalid_document_error)?;
        }
        Block::Cut { partial } => {
            if partial.unwrap_or(false) {
                printer.partial_cut().map_err(print_error)?;
            } else {
                printer.cut().map_err(print_error)?;
            }
        }
    }
    Ok(())
}

fn render_columns<D: Driver>(
    printer: &mut Printer<D>,
    columns: &[Column],
    line_width: usize,
) -> Result<(), EscposError> {
    for (index, (column, text)) in column_segments(columns, line_width).into_iter().enumerate() {
        apply_style(printer, column.style.as_ref())?;
        write_encoded(printer, &text)?;
        if index + 1 == columns.len() {
            printer.feed().map_err(print_error)?;
        }
    }
    Ok(())
}

fn writeln_encoded<D: Driver>(printer: &mut Printer<D>, text: &str) -> Result<(), EscposError> {
    write_encoded(printer, text)?;
    printer.feed().map_err(print_error)?;
    Ok(())
}

fn write_encoded<D: Driver>(printer: &mut Printer<D>, text: &str) -> Result<(), EscposError> {
    printer.custom(&encode_pc858(text)).map_err(print_error)?;
    Ok(())
}

fn encode_pc858(text: &str) -> Vec<u8> {
    text.chars().map(encode_pc858_char).collect()
}

fn encode_pc858_char(c: char) -> u8 {
    if (c as u32) < 0x80 {
        c as u8
    } else {
        encode_pc858_high(c)
    }
}

fn encode_pc858_high(c: char) -> u8 {
    encode_pc858_currency(c).unwrap_or_else(|| encode_pc858_latin(c))
}

fn encode_pc858_currency(c: char) -> Option<u8> {
    match c {
        '€' => Some(0xD5),
        '£' => Some(0x9C),
        '¥' => Some(0xBE),
        '¢' => Some(0xBD),
        _ => None,
    }
}

fn encode_pc858_latin(c: char) -> u8 {
    match c {
        'á' => 0xA0,
        'é' => 0x82,
        'í' => 0xA1,
        'ó' => 0xA2,
        'ú' => 0xA3,
        'ñ' => 0xA4,
        'Ñ' => 0xA5,
        '¡' => 0xAD,
        '¿' => 0xA8,
        'ü' => 0x81,
        'ö' => 0x94,
        'ä' => 0x84,
        'à' => 0x85,
        'è' => 0x8A,
        'ç' => 0x87,
        _ => b'?',
    }
}

fn apply_style<D: Driver>(
    printer: &mut Printer<D>,
    style: Option<&TextStyle>,
) -> Result<(), EscposError> {
    reset_style(printer)?;
    let Some(style) = style else {
        return Ok(());
    };

    printer
        .bold(style.bold.unwrap_or(false))
        .and_then(|printer| {
            printer.underline(if style.underline.unwrap_or(false) {
                UnderlineMode::Single
            } else {
                UnderlineMode::None
            })
        })
        .map_err(print_error)?;

    let (width, height) = match style.size.unwrap_or(FontSize::Normal) {
        FontSize::Normal => (1, 1),
        FontSize::Wide => (2, 1),
        FontSize::Tall => (1, 2),
        FontSize::Double => (2, 2),
    };
    printer.size(width, height).map_err(print_error)?;
    set_alignment(printer, style.align.unwrap_or(Align::Left))
}

fn reset_style<D: Driver>(printer: &mut Printer<D>) -> Result<(), EscposError> {
    printer
        .bold(false)
        .and_then(|printer| printer.underline(UnderlineMode::None))
        .and_then(|printer| printer.size(1, 1))
        .and_then(|printer| printer.justify(JustifyMode::LEFT))
        .map_err(print_error)?;
    Ok(())
}

fn set_alignment<D: Driver>(
    printer: &mut Printer<D>,
    align: Align,
) -> Result<(), EscposError> {
    let mode = match align {
        Align::Left => JustifyMode::LEFT,
        Align::Center => JustifyMode::CENTER,
        Align::Right => JustifyMode::RIGHT,
    };
    printer.justify(mode).map_err(print_error)?;
    Ok(())
}

fn horizontal_scale(style: Option<&TextStyle>) -> usize {
    match style.and_then(|style| style.size) {
        Some(FontSize::Wide | FontSize::Double) => 2,
        _ => 1,
    }
}

pub(crate) fn wrap_text(value: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![value.to_string()];
    }

    let mut result = Vec::new();
    for source_line in value.lines() {
        let mut line = String::new();
        for word in source_line.split_whitespace() {
            if word.chars().count() > width {
                if !line.is_empty() {
                    result.push(std::mem::take(&mut line));
                }
                let chars: Vec<char> = word.chars().collect();
                result.extend(
                    chars
                        .chunks(width)
                        .map(|chunk| chunk.iter().collect::<String>()),
                );
            } else if line.is_empty() {
                line.push_str(word);
            } else if line.chars().count() + 1 + word.chars().count() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                result.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            result.push(line);
        } else if source_line.is_empty() {
            result.push(String::new());
        }
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

fn column_segments<'a>(columns: &'a [Column], line_width: usize) -> Vec<(&'a Column, String)> {
    let explicit_width: usize = columns
        .iter()
        .filter_map(|column| column.width)
        .map(|width| (width * line_width as f32).round() as usize)
        .sum();
    let unspecified_count = columns
        .iter()
        .filter(|column| column.width.is_none())
        .count();
    let remaining = line_width.saturating_sub(explicit_width);
    let default_width = if unspecified_count == 0 {
        0
    } else {
        remaining / unspecified_count
    };

    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let physical_width = column
                .width
                .map(|value| (value * line_width as f32).round() as usize)
                .unwrap_or(default_width);
            let width = physical_width / horizontal_scale(column.style.as_ref());
            let align = column
                .align
                .unwrap_or(if index == 0 { Align::Left } else { Align::Right });
            (column, fit_text(&column.text, width, align))
        })
        .collect()
}

fn fit_text(value: &str, width: usize, align: Align) -> String {
    let value: String = value.chars().take(width).collect();
    let padding = width.saturating_sub(value.chars().count());
    match align {
        Align::Left => format!("{value}{}", " ".repeat(padding)),
        Align::Center => {
            let left = padding / 2;
            format!("{}{}{}", " ".repeat(left), value, " ".repeat(padding - left))
        }
        Align::Right => format!("{}{value}", " ".repeat(padding)),
    }
}

pub(crate) fn decode_image(data: &str) -> Result<Vec<u8>, EscposError> {
    let encoded = data.split_once(',').map_or(data, |(_, encoded)| encoded);
    STANDARD
        .decode(encoded)
        .map_err(|error| EscposError::invalid_document(format!("Invalid base64 image: {error}")))
}

fn invalid_document_error(error: impl std::fmt::Display) -> EscposError {
    EscposError::new(ErrorCode::InvalidDocument, error.to_string())
}

fn print_error(error: impl std::fmt::Display) -> EscposError {
    EscposError::new(ErrorCode::PrintFailed, error.to_string())
}
