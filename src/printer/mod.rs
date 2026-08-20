use crate::models::{
    Align, BarcodeSymbology, Block, Column, ErrorCode, EscposError, PrintDocument, PrinterBackend,
    PrinterInfo, PrinterTarget, TextStyle,
};
use escpos::driver::{Driver, NetworkDriver};

use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

#[path = "../barcode.rs"]
mod barcode;
#[path = "../raster.rs"]
mod raster;
pub(crate) mod render;
pub(crate) mod text;

#[cfg(target_os = "windows")]
use escpos::driver::WindowsUsbPrintDriver;

const DEFAULT_NETWORK_HOST: &str = "127.0.0.1";
const DEFAULT_NETWORK_PORT: u16 = 9100;
const NETWORK_OPEN_TIMEOUT: Duration = Duration::from_secs(2);
const NETWORK_PROBE_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicsBackend {
    Escpos,
    Emulator,
}

use render::render_with_graphics;

pub fn print_document(
    target: &PrinterTarget,
    document: &PrintDocument,
) -> Result<(), EscposError> {
    match target {
        PrinterTarget::WindowsUsbByPath { path } => {
            #[cfg(target_os = "windows")]
            {
                let driver = WindowsUsbPrintDriver::open(path).map_err(|error| {
                    EscposError::new(
                        ErrorCode::OpenFailed,
                        format!("Failed to open Windows USB printer: {error}"),
                    )
                })?;
                render_with_graphics(driver, document, GraphicsBackend::Escpos)
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = path;
                Err(unsupported_platform())
            }
        }
        PrinterTarget::WindowsUsbByVidPid {
            vendor_id,
            product_id,
        } => {
            #[cfg(target_os = "windows")]
            {
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
                render_with_graphics(driver, document, GraphicsBackend::Escpos)
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = (vendor_id, product_id);
                Err(unsupported_platform())
            }
        }
        PrinterTarget::Network { host, port } => {
            let timeout = host.parse::<std::net::IpAddr>().ok().map(|_| NETWORK_OPEN_TIMEOUT);
            let driver = NetworkDriver::open(host, *port, timeout).map_err(|error| {
                EscposError::new(
                    ErrorCode::OpenFailed,
                    format!("Failed to open network printer {host}:{port}: {error}"),
                )
            })?;
            render_with_graphics(driver, document, graphics_backend_for(target))
        }
    }
}

pub fn list_printers() -> Result<Vec<PrinterInfo>, EscposError> {
    let mut printers = list_usb_printers()?;
    printers.extend(list_network_printers());
    Ok(printers)
}

#[cfg(target_os = "windows")]
fn list_usb_printers() -> Result<Vec<PrinterInfo>, EscposError> {
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
                    host: None,
                    port: None,
                    backend: PrinterBackend::WindowsUsb,
                })
                .collect()
        })
}

#[cfg(not(target_os = "windows"))]
fn list_usb_printers() -> Result<Vec<PrinterInfo>, EscposError> {
    Ok(Vec::new())
}

fn list_network_printers() -> Vec<PrinterInfo> {
    network_endpoints()
        .into_iter()
        .map(|(host, port)| PrinterInfo::network(host, port))
        .collect()
}

fn network_endpoints() -> Vec<(String, u16)> {
    let mut endpoints = configured_network_endpoints();

    if !endpoints
        .iter()
        .any(|(host, port)| host == DEFAULT_NETWORK_HOST && *port == DEFAULT_NETWORK_PORT)
        && should_include_default_emulator()
    {
        endpoints.push((DEFAULT_NETWORK_HOST.to_string(), DEFAULT_NETWORK_PORT));
    }

    endpoints
}

fn configured_network_endpoints() -> Vec<(String, u16)> {
    std::env::var("ESCPOS_NETWORK_PRINTERS")
        .ok()
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .filter_map(|part| parse_host_port(part.trim()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn should_include_default_emulator() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }

    let address = SocketAddr::from(([127, 0, 0, 1], DEFAULT_NETWORK_PORT));
    TcpStream::connect_timeout(&address, NETWORK_PROBE_TIMEOUT).is_ok()
}

pub(crate) fn parse_host_port(value: &str) -> Option<(String, u16)> {
    let (host, port) = value.rsplit_once(':')?;
    if host.is_empty() {
        return None;
    }
    let port = port.parse().ok().filter(|port| *port != 0)?;
    Some((host.to_string(), port))
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

pub fn self_test() -> Result<Vec<PrinterInfo>, EscposError> {
    let printers = list_printers()?;
    let printer = printers
        .iter()
        .find(|printer| matches!(printer.backend, PrinterBackend::Network))
        .or_else(|| printers.first())
        .ok_or_else(|| {
            EscposError::new(
                ErrorCode::PrinterNotFound,
                "No ESC/POS printer was found. Start a TCP emulator on 127.0.0.1:9100 or connect a USB printer.",
            )
        })?;
    test_printer(&printer.to_target()?)?;
    Ok(printers)
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
    render_with_graphics(driver, document, GraphicsBackend::Escpos)
}

pub(crate) fn graphics_backend_for(target: &PrinterTarget) -> GraphicsBackend {
    match target {
        PrinterTarget::Network { host, .. } if is_local_emulator_host(host) => {
            GraphicsBackend::Emulator
        }
        _ => GraphicsBackend::Escpos,
    }
}

fn is_local_emulator_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn invalid_document_error(error: impl std::fmt::Display) -> EscposError {
    EscposError::new(ErrorCode::InvalidDocument, error.to_string())
}

fn print_error(error: impl std::fmt::Display) -> EscposError {
    EscposError::new(ErrorCode::PrintFailed, error.to_string())
}
