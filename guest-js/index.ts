import { invoke } from '@tauri-apps/api/core'

export type Align = 'left' | 'center' | 'right'
export type FontSize = 'normal' | 'wide' | 'tall' | 'double'
export type CharacterSet = 'PC858'

export type TextStyle = {
  bold?: boolean
  underline?: boolean
  size?: FontSize
  align?: Align
}

export type Column = {
  text: string
  width?: number
  align?: Align
  style?: TextStyle
}

export type Block =
  | { type: 'text'; value: string; style?: TextStyle }
  | { type: 'feed'; lines?: number }
  | { type: 'divider'; char?: string }
  | { type: 'columns'; columns: Column[] }
  | {
      type: 'image'
      data: string
      mime: 'image/png' | 'image/jpeg'
      maxWidthDots?: number
      align?: Align
    }
  | {
      type: 'barcode'
      value: string
      symbology: 'ean13' | 'ean8' | 'upca' | 'code39'
      align?: Align
      printValue?: boolean
    }
  | { type: 'qr'; value: string; size?: number; align?: Align }
  | { type: 'cut'; partial?: boolean }

export type PrintDocument = {
  paperWidthMm?: 58 | 80
  characterSet?: CharacterSet
  blocks: Block[]
}

export type PrinterTarget =
  | { kind: 'windows_usb'; path: string }
  | { kind: 'windows_usb'; vendorId: number; productId: number }

export type PrinterInfo = {
  id: string
  path: string
  name?: string
  vendorId?: number
  productId?: number
  backend: 'windows_usb'
}

export type ErrorCode =
  | 'unsupported_platform'
  | 'printer_not_found'
  | 'open_failed'
  | 'invalid_document'
  | 'print_failed'

export type EscposError = {
  code: ErrorCode
  message: string
}

export type PrintOptions = {
  printer: PrinterTarget
  document: PrintDocument
}

export async function print(options: PrintOptions): Promise<void> {
  return invoke('plugin:escpos|print', options)
}

export async function listPrinters(): Promise<PrinterInfo[]> {
  return invoke('plugin:escpos|list_printers')
}

export async function testPrinter(options: { printer: PrinterTarget }): Promise<void> {
  return invoke('plugin:escpos|test_printer', options)
}
