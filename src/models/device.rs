use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterTarget {
    WindowsUsbByPath { path: String },
    WindowsUsbByVidPid { vendor_id: u16, product_id: u16 },
    Network { host: String, port: u16 },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrinterTargetWire {
    kind: PrinterBackend,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendor_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    product_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
}

impl Serialize for PrinterTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let wire = match self {
            Self::WindowsUsbByPath { path } => PrinterTargetWire {
                kind: PrinterBackend::WindowsUsb,
                path: Some(path.clone()),
                vendor_id: None,
                product_id: None,
                host: None,
                port: None,
            },
            Self::WindowsUsbByVidPid {
                vendor_id,
                product_id,
            } => PrinterTargetWire {
                kind: PrinterBackend::WindowsUsb,
                path: None,
                vendor_id: Some(*vendor_id),
                product_id: Some(*product_id),
                host: None,
                port: None,
            },
            Self::Network { host, port } => PrinterTargetWire {
                kind: PrinterBackend::Network,
                path: None,
                vendor_id: None,
                product_id: None,
                host: Some(host.clone()),
                port: Some(*port),
            },
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PrinterTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PrinterTargetWire::deserialize(deserializer)?;
        match wire.kind {
            PrinterBackend::WindowsUsb => match (wire.path, wire.vendor_id, wire.product_id) {
                (Some(path), None, None) if !path.is_empty() => Ok(Self::WindowsUsbByPath { path }),
                (None, Some(vendor_id), Some(product_id)) => Ok(Self::WindowsUsbByVidPid {
                    vendor_id,
                    product_id,
                }),
                _ => Err(serde::de::Error::custom(
                    "windows_usb target requires either path or vendorId and productId",
                )),
            },
            PrinterBackend::Network => match (wire.host, wire.port) {
                (Some(host), Some(port)) if !host.is_empty() && port != 0 => {
                    Ok(Self::Network { host, port })
                }
                _ => Err(serde::de::Error::custom(
                    "network target requires host and a non-zero port",
                )),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrinterBackend {
    WindowsUsb,
    Network,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    pub id: String,
    pub path: String,
    pub name: Option<String>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub backend: PrinterBackend,
}

impl PrinterInfo {
    pub fn network(host: impl Into<String>, port: u16) -> Self {
        let host = host.into();
        let path = format!("{host}:{port}");
        Self {
            id: format!("network:{path}"),
            path,
            name: Some(format!("ESC/POS network ({host}:{port})")),
            vendor_id: None,
            product_id: None,
            host: Some(host),
            port: Some(port),
            backend: PrinterBackend::Network,
        }
    }

    pub fn to_target(&self) -> Result<PrinterTarget, EscposError> {
        match self.backend {
            PrinterBackend::WindowsUsb => Ok(PrinterTarget::WindowsUsbByPath {
                path: self.path.clone(),
            }),
            PrinterBackend::Network => {
                let host = self.host.clone().filter(|value| !value.is_empty()).ok_or_else(
                    || {
                        EscposError::new(
                            ErrorCode::PrinterNotFound,
                            "Network printer is missing a host",
                        )
                    },
                )?;
                let port = self.port.filter(|value| *value != 0).ok_or_else(|| {
                    EscposError::new(
                        ErrorCode::PrinterNotFound,
                        "Network printer is missing a port",
                    )
                })?;
                Ok(PrinterTarget::Network { host, port })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnsupportedPlatform,
    PrinterNotFound,
    OpenFailed,
    InvalidDocument,
    PrintFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct EscposError {
    pub code: ErrorCode,
    pub message: String,
}

impl EscposError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_document(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidDocument, message)
    }
}

impl std::fmt::Display for EscposError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for EscposError {}
