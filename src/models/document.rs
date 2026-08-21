use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::EscposError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperWidth {
    Mm58,
    Mm80,
}

impl PaperWidth {
    pub const fn characters_per_line(self) -> usize {
        match self {
            Self::Mm58 => 32,
            Self::Mm80 => 48,
        }
    }
}

impl Serialize for PaperWidth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(match self {
            Self::Mm58 => 58,
            Self::Mm80 => 80,
        })
    }
}

impl<'de> Deserialize<'de> for PaperWidth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u16::deserialize(deserializer)? {
            58 => Ok(Self::Mm58),
            80 => Ok(Self::Mm80),
            value => Err(serde::de::Error::custom(format!(
                "unsupported paper width: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum CharacterSet {
    #[serde(rename = "PC858")]
    Pc858,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontSize {
    Small,
    Normal,
    Wide,
    Tall,
    Double,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    pub bold: Option<bool>,
    pub underline: Option<bool>,
    pub size: Option<FontSize>,
    pub align: Option<Align>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub text: String,
    pub width: Option<f32>,
    pub align: Option<Align>,
    pub style: Option<TextStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageMime {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BarcodeSymbology {
    Ean13,
    Ean8,
    Upca,
    Code39,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Block {
    Text {
        value: String,
        style: Option<TextStyle>,
    },
    Feed {
        lines: Option<u8>,
    },
    Divider {
        #[serde(rename = "char")]
        character: Option<String>,
    },
    Columns {
        columns: Vec<Column>,
    },
    Image {
        data: String,
        mime: ImageMime,
        max_width_dots: Option<u16>,
        align: Option<Align>,
    },
    Barcode {
        value: String,
        symbology: BarcodeSymbology,
        align: Option<Align>,
        print_value: Option<bool>,
    },
    Qr {
        value: String,
        size: Option<u8>,
        align: Option<Align>,
    },
    Cut {
        partial: Option<bool>,
    },
}

impl Block {
    pub fn validate(&self) -> Result<(), EscposError> {
        match self {
            Self::Columns { columns } if columns.is_empty() => Err(EscposError::invalid_document(
                "Columns block must contain at least one column",
            )),
            Self::Columns { columns } => validate_columns(columns),
            Self::Divider {
                character: Some(character),
            } if character.chars().count() != 1 => Err(EscposError::invalid_document(
                "Divider character must contain exactly one character",
            )),
            Self::Image { data, .. } if data.trim().is_empty() => {
                Err(EscposError::invalid_document("Image data must not be empty"))
            }
            Self::Barcode {
                value,
                symbology: BarcodeSymbology::Ean13,
                ..
            } if !is_numeric_length(value, &[12, 13]) => Err(EscposError::invalid_document(
                "EAN-13 barcode must contain 12 or 13 digits",
            )),
            Self::Barcode {
                value,
                symbology: BarcodeSymbology::Ean8,
                ..
            } if !is_numeric_length(value, &[7, 8]) => Err(EscposError::invalid_document(
                "EAN-8 barcode must contain 7 or 8 digits",
            )),
            Self::Barcode {
                value,
                symbology: BarcodeSymbology::Upca,
                ..
            } if !is_numeric_length(value, &[11, 12]) => Err(EscposError::invalid_document(
                "UPC-A barcode must contain 11 or 12 digits",
            )),
            Self::Barcode {
                value,
                symbology: BarcodeSymbology::Code39,
                ..
            } if value.is_empty() => Err(EscposError::invalid_document(
                "CODE39 barcode must not be empty",
            )),
            Self::Qr { value, .. } if value.is_empty() => {
                Err(EscposError::invalid_document("QR code must not be empty"))
            }
            Self::Qr {
                size: Some(size), ..
            } if !(1..=16).contains(size) => Err(EscposError::invalid_document(
                "QR code size must be between 1 and 16",
            )),
            _ => Ok(()),
        }
    }
}

fn validate_columns(columns: &[Column]) -> Result<(), EscposError> {
    let mut total_width = 0.0_f32;
    for column in columns {
        if let Some(width) = column.width {
            if !width.is_finite() || width <= 0.0 || width > 1.0 {
                return Err(EscposError::invalid_document(
                    "Column width must be greater than 0 and no greater than 1",
                ));
            }
            total_width += width;
        }
    }
    if total_width > 1.0 + f32::EPSILON {
        return Err(EscposError::invalid_document(
            "Column widths must not exceed 1",
        ));
    }
    Ok(())
}

fn is_numeric_length(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintDocument {
    pub paper_width_mm: Option<PaperWidth>,
    pub character_set: Option<CharacterSet>,
    pub blocks: Vec<Block>,
}

impl PrintDocument {
    pub fn paper_width_mm(&self) -> PaperWidth {
        self.paper_width_mm.unwrap_or(PaperWidth::Mm80)
    }

    pub fn character_set(&self) -> CharacterSet {
        self.character_set.unwrap_or(CharacterSet::Pc858)
    }

    pub fn characters_per_line(&self) -> usize {
        self.paper_width_mm().characters_per_line()
    }

    pub fn validate(&self) -> Result<(), EscposError> {
        for block in &self.blocks {
            block.validate()?;
        }
        Ok(())
    }
}
