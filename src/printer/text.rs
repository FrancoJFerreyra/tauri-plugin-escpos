use crate::models::{Align, Column, EscposError, FontSize, TextStyle};
use escpos::{driver::Driver, printer::Printer, utils::{JustifyMode, UnderlineMode}};

use super::print_error;

pub(super) fn writeln_encoded<D: Driver>(
    printer: &mut Printer<D>,
    text: &str,
) -> Result<(), EscposError> {
    write_encoded(printer, text)?;
    write_line_feed(printer)
}

fn write_encoded<D: Driver>(printer: &mut Printer<D>, text: &str) -> Result<(), EscposError> {
    printer.custom(&encode_pc858(text)).map_err(print_error)?;
    Ok(())
}

pub(super) fn write_line_feeds<D: Driver>(
    printer: &mut Printer<D>,
    lines: u8,
) -> Result<(), EscposError> {
    for _ in 0..lines {
        write_line_feed(printer)?;
    }
    Ok(())
}

pub(super) fn write_line_feed<D: Driver>(printer: &mut Printer<D>) -> Result<(), EscposError> {
    printer.custom(&[0x0A]).map_err(print_error)?;
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

pub(super) fn apply_style<D: Driver>(
    printer: &mut Printer<D>,
    style: Option<&TextStyle>,
) -> Result<(), EscposError> {
    reset_style(printer)?;
    let Some(style) = style else {
        return Ok(());
    };
    apply_font_style(printer, Some(style))?;
    set_alignment(printer, style.align.unwrap_or(Align::Left))
}

pub(super) fn apply_font_style<D: Driver>(
    printer: &mut Printer<D>,
    style: Option<&TextStyle>,
) -> Result<(), EscposError> {
    let (width, height) = font_dots(style.and_then(|style| style.size));
    printer
        .bold(style.and_then(|style| style.bold).unwrap_or(false))
        .and_then(|printer| {
            printer.underline(if style.and_then(|style| style.underline).unwrap_or(false) {
                UnderlineMode::Single
            } else {
                UnderlineMode::None
            })
        })
        .and_then(|printer| printer.size(width, height))
        .map_err(print_error)?;
    Ok(())
}

fn font_dots(size: Option<FontSize>) -> (u8, u8) {
    match size.unwrap_or(FontSize::Normal) {
        FontSize::Normal => (1, 1),
        FontSize::Wide => (2, 1),
        FontSize::Tall => (1, 2),
        FontSize::Double => (2, 2),
    }
}

pub(super) fn reset_style<D: Driver>(printer: &mut Printer<D>) -> Result<(), EscposError> {
    printer
        .bold(false)
        .and_then(|printer| printer.underline(UnderlineMode::None))
        .and_then(|printer| printer.size(1, 1))
        .and_then(|printer| printer.justify(JustifyMode::LEFT))
        .map_err(print_error)?;
    Ok(())
}

pub(super) fn set_alignment<D: Driver>(
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

pub(super) fn horizontal_scale(style: Option<&TextStyle>) -> usize {
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

pub(super) fn column_segments<'a>(
    columns: &'a [Column],
    line_width: usize,
) -> Vec<(&'a Column, String)> {
    let widths = padded_column_widths(columns, line_width);
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let align = column
                .align
                .unwrap_or(if index == 0 { Align::Left } else { Align::Right });
            (column, fit_text(&column.text, widths[index], align))
        })
        .collect()
}

fn padded_column_widths(columns: &[Column], line_width: usize) -> Vec<usize> {
    let mut widths = physical_column_widths(columns, line_width);
    let used: usize = widths.iter().sum();
    if let Some(last) = widths.last_mut() {
        *last += line_width.saturating_sub(used);
    }
    widths
}

fn physical_column_widths(columns: &[Column], line_width: usize) -> Vec<usize> {
    let explicit_width: usize = columns
        .iter()
        .filter_map(|column| column.width)
        .map(|width| (width * line_width as f32).round() as usize)
        .sum();
    let unspecified_count = columns.iter().filter(|column| column.width.is_none()).count();
    let default_width = if unspecified_count == 0 {
        0
    } else {
        line_width.saturating_sub(explicit_width) / unspecified_count
    };
    columns
        .iter()
        .map(|column| {
            column
                .width
                .map(|value| (value * line_width as f32).round() as usize)
                .unwrap_or(default_width)
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
