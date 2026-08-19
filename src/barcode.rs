use image::{GrayImage, Luma};

use crate::models::EscposError;

use super::raster;

const LEFT_ODD: [&str; 10] = [
    "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
    "0110111", "0001011",
];
const LEFT_EVEN: [&str; 10] = [
    "0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001",
    "0001001", "0010111",
];
const RIGHT: [&str; 10] = [
    "1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100",
    "1001000", "1110100",
];
const LEFT_PARITY: [&str; 10] = [
    "LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL",
    "LGGLGL",
];

pub fn ean13_command(value: &str) -> Result<(Vec<u8>, String), EscposError> {
    let digits = normalize_ean13(value)?;
    let text: String = digits.iter().map(|digit| char::from(b'0' + digit)).collect();
    Ok((raster::gs_v0_from_gray(&render_ean13(&digits)), text))
}

pub fn function_b_command(system: u8, value: &str) -> Vec<u8> {
    let mut command = vec![0x1D, b'k', system, value.len() as u8];
    command.extend(value.as_bytes());
    command
}

fn normalize_ean13(value: &str) -> Result<[u8; 13], EscposError> {
    let digits = parse_digits(value)?;
    match digits.len() {
        12 => Ok(with_check_digit(&digits)),
        13 => Ok(to_thirteen(&digits)),
        _ => Err(EscposError::invalid_document(
            "EAN-13 barcode must contain 12 or 13 digits",
        )),
    }
}

fn render_ean13(digits: &[u8; 13]) -> GrayImage {
    draw_modules(&ean13_modules(digits), 2, 64)
}

fn parse_digits(value: &str) -> Result<Vec<u8>, EscposError> {
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EscposError::invalid_document(
            "EAN-13 barcode must contain only digits",
        ));
    }
    Ok(value.bytes().map(|byte| byte - b'0').collect())
}

fn with_check_digit(digits: &[u8]) -> [u8; 13] {
    let mut out = [0u8; 13];
    out[..12].copy_from_slice(digits);
    out[12] = check_digit(digits);
    out
}

fn to_thirteen(digits: &[u8]) -> [u8; 13] {
    let mut out = [0u8; 13];
    out.copy_from_slice(digits);
    out
}

fn check_digit(digits: &[u8]) -> u8 {
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(index, digit)| {
            if index % 2 == 0 {
                u32::from(*digit)
            } else {
                u32::from(*digit) * 3
            }
        })
        .sum();
    ((10 - (sum % 10)) % 10) as u8
}

fn ean13_modules(digits: &[u8; 13]) -> String {
    let mut modules = String::from("000000000101");
    append_left(&mut modules, digits);
    modules.push_str("01010");
    append_right(&mut modules, digits);
    modules.push_str("101000000000");
    modules
}

fn append_left(modules: &mut String, digits: &[u8; 13]) {
    let parity = LEFT_PARITY[digits[0] as usize];
    for (index, flag) in parity.chars().enumerate() {
        let digit = digits[index + 1] as usize;
        modules.push_str(if flag == 'L' {
            LEFT_ODD[digit]
        } else {
            LEFT_EVEN[digit]
        });
    }
}

fn append_right(modules: &mut String, digits: &[u8; 13]) {
    for digit in &digits[7..] {
        modules.push_str(RIGHT[*digit as usize]);
    }
}

fn draw_modules(modules: &str, scale: u32, height: u32) -> GrayImage {
    let width = modules.len() as u32 * scale;
    let mut gray = GrayImage::from_pixel(width, height, Luma([255]));
    for (index, module) in modules.chars().enumerate() {
        if module == '1' {
            fill_bar(&mut gray, index as u32 * scale, scale, height);
        }
    }
    gray
}

fn fill_bar(gray: &mut GrayImage, x0: u32, width: u32, height: u32) {
    for y in 0..height {
        for x in x0..x0 + width {
            gray.put_pixel(x, y, Luma([0]));
        }
    }
}
