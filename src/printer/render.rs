use crate::models::{
    Align, BarcodeSymbology, Block, CharacterSet, Column, EscposError, PrintDocument,
};
use escpos::{
    driver::Driver,
    printer::Printer,
    utils::{
        BarcodeFont, BarcodeHeight, BarcodeOption, BarcodePosition, BarcodeWidth, BitImageOption,
        BitImageSize, PageCode, Protocol, QRCodeCorrectionLevel, QRCodeModel, QRCodeOption,
    },
};

use crate::virtual_printer::barcode;
use crate::virtual_printer::raster;
use super::text::{
    apply_font_style, apply_style, column_rows, horizontal_scale, reset_style, set_alignment,
    wrap_text, write_line_feed, writeln_encoded,
};
use super::{invalid_document_error, print_error, GraphicsBackend};
use base64::{engine::general_purpose::STANDARD, Engine as _};

pub fn render_with_graphics<D: Driver>(
    driver: D,
    document: &PrintDocument,
    graphics: GraphicsBackend,
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
        render_block(&mut printer, block, line_width, graphics)?;
    }

    printer.print().map_err(print_error)?;
    Ok(())
}

fn render_block<D: Driver>(
    printer: &mut Printer<D>,
    block: &Block,
    line_width: usize,
    graphics: GraphicsBackend,
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
            reset_style(printer)?;
            printer
                .feeds(lines.unwrap_or(1))
                .map_err(print_error)?;
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
            render_image(printer, data, *max_width_dots, graphics)?;
        }
        Block::Barcode {
            value,
            symbology,
            align,
            print_value,
        } => {
            set_alignment(printer, align.unwrap_or(Align::Center))?;
            render_barcode(
                printer,
                value,
                *symbology,
                print_value.unwrap_or(true),
                graphics,
            )?;
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

fn render_image<D: Driver>(
    printer: &mut Printer<D>,
    data: &str,
    max_width_dots: Option<u16>,
    graphics: GraphicsBackend,
) -> Result<(), EscposError> {
    let bytes = decode_image(data)?;
    match graphics {
        GraphicsBackend::Emulator => {
            let command = raster::gs_v0_command(&bytes, max_width_dots)?;
            printer.custom(&command).map_err(print_error)?;
            write_line_feed(printer)
        }
        GraphicsBackend::Escpos => {
            let max_width = max_width_dots
                .map(|width| u32::from(width.max(8) / 8 * 8))
                .or(Some(512));
            let option = BitImageOption::new(max_width, None, BitImageSize::Normal)
                .map_err(invalid_document_error)?;
            printer
                .bit_image_from_bytes_option(&bytes, option)
                .map_err(invalid_document_error)?;
            Ok(())
        }
    }
}

fn render_columns<D: Driver>(
    printer: &mut Printer<D>,
    columns: &[Column],
    line_width: usize,
) -> Result<(), EscposError> {
    reset_style(printer)?;
    apply_font_style(printer, columns.first().and_then(|column| column.style.as_ref()))?;
    for line in column_rows(columns, line_width) {
        writeln_encoded(printer, &line)?;
    }
    Ok(())
}

fn render_barcode<D: Driver>(
    printer: &mut Printer<D>,
    value: &str,
    symbology: BarcodeSymbology,
    print_value: bool,
    graphics: GraphicsBackend,
) -> Result<(), EscposError> {
    match graphics {
        GraphicsBackend::Emulator => render_emulator_barcode(printer, value, symbology, print_value),
        GraphicsBackend::Escpos => render_escpos_barcode(printer, value, symbology, print_value),
    }
}

fn render_escpos_barcode<D: Driver>(
    printer: &mut Printer<D>,
    value: &str,
    symbology: BarcodeSymbology,
    print_value: bool,
) -> Result<(), EscposError> {
    let position = if print_value {
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
    Ok(())
}

fn render_emulator_barcode<D: Driver>(
    printer: &mut Printer<D>,
    value: &str,
    symbology: BarcodeSymbology,
    print_value: bool,
) -> Result<(), EscposError> {
    match symbology {
        BarcodeSymbology::Ean13 => render_emulator_ean13(printer, value, print_value),
        BarcodeSymbology::Ean8 => render_function_b(printer, 68, value, print_value),
        BarcodeSymbology::Upca => render_function_b(printer, 65, value, print_value),
        BarcodeSymbology::Code39 => render_function_b(printer, 69, value, print_value),
    }
}

fn render_emulator_ean13<D: Driver>(
    printer: &mut Printer<D>,
    value: &str,
    print_value: bool,
) -> Result<(), EscposError> {
    let (command, text) = barcode::ean13_command(value)?;
    printer.custom(&command).map_err(print_error)?;
    write_line_feed(printer)?;
    if print_value {
        writeln_encoded(printer, &text)?;
    }
    Ok(())
}

fn render_function_b<D: Driver>(
    printer: &mut Printer<D>,
    system: u8,
    value: &str,
    print_value: bool,
) -> Result<(), EscposError> {
    printer.custom(&[0x1D, b'h', 80]).map_err(print_error)?;
    printer.custom(&[0x1D, b'w', 2]).map_err(print_error)?;
    let hri = if print_value { 2 } else { 0 };
    printer.custom(&[0x1D, b'H', hri]).map_err(print_error)?;
    printer
        .custom(&barcode::function_b_command(system, value))
        .map_err(print_error)?;
    Ok(())
}

pub(crate) fn decode_image(data: &str) -> Result<Vec<u8>, EscposError> {
    let encoded = data.split_once(',').map_or(data, |(_, encoded)| encoded);
    STANDARD
        .decode(encoded)
        .map_err(|error| EscposError::invalid_document(format!("Invalid base64 image: {error}")))
}
