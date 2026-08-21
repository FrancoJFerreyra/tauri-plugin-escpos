use image::{imageops::FilterType, DynamicImage, GrayImage};

use crate::models::EscposError;

pub fn gs_v0_command(bytes: &[u8], max_width_dots: Option<u16>) -> Result<Vec<u8>, EscposError> {
    let gray = load_gray_bitmap(bytes, max_width_dots)?;
    Ok(gs_v0_from_gray(&gray))
}

fn load_gray_bitmap(bytes: &[u8], max_width_dots: Option<u16>) -> Result<GrayImage, EscposError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| EscposError::invalid_document(format!("Invalid image: {error}")))?;
    let max_width = max_width_dots
        .map(|width| u32::from(width.max(8) / 8 * 8))
        .unwrap_or(384);
    Ok(resize_to_width(image, max_width).to_luma8())
}

pub(crate) fn gs_v0_from_gray(gray: &GrayImage) -> Vec<u8> {
    encode_gs_v0(gray)
}

fn encode_gs_v0(gray: &GrayImage) -> Vec<u8> {
    let (width, height) = padded_size(gray.width(), gray.height());
    let bytes_per_row = (width / 8) as u16;
    let mut command = gs_v0_header(bytes_per_row, height as u16);
    command.extend(raster_bytes(gray, height, bytes_per_row));
    command
}

fn resize_to_width(image: DynamicImage, max_width: u32) -> DynamicImage {
    if image.width() <= max_width {
        return image;
    }
    let height = image.height().saturating_mul(max_width) / image.width().max(1);
    image.resize(max_width, height.max(1), FilterType::Triangle)
}

fn padded_size(width: u32, height: u32) -> (u32, u32) {
    (pad8(width.max(1)), pad8(height.max(1)))
}

fn gs_v0_header(bytes_per_row: u16, height: u16) -> Vec<u8> {
    vec![
        0x1D,
        b'v',
        b'0',
        0,
        bytes_per_row as u8,
        (bytes_per_row >> 8) as u8,
        height as u8,
        (height >> 8) as u8,
    ]
}

fn raster_bytes(gray: &GrayImage, height: u32, bytes_per_row: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity((bytes_per_row as u32 * height) as usize);
    for y in 0..height {
        for column in 0..bytes_per_row {
            data.push(pack_row_byte(gray, y, column));
        }
    }
    data
}

fn pad8(value: u32) -> u32 {
    (value + 7) / 8 * 8
}

fn pack_row_byte(gray: &GrayImage, y: u32, column: u16) -> u8 {
    let mut byte = 0u8;
    for bit in 0..8 {
        if is_black(gray, u32::from(column) * 8 + bit, y) {
            byte |= 0x80 >> bit;
        }
    }
    byte
}

fn is_black(gray: &GrayImage, x: u32, y: u32) -> bool {
    x < gray.width() && y < gray.height() && gray.get_pixel(x, y)[0] < 128
}
