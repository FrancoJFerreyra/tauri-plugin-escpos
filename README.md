# tauri-plugin-escpos

A Tauri 2 plugin for printing structured ESC/POS documents directly to thermal printers.

The plugin exposes printer primitives instead of receipt-specific fields. Applications remain
responsible for business data, translations, currencies, and layout composition.

## Platform support

Version 0.1 supports USB printers on Windows through the standard `usbprint.sys` driver. It does
not require replacing the printer driver with Zadig, WinUSB, or libusb.

Network printers (raw TCP, typically port 9100) are supported on all platforms. This is the path
to use with emulators such as [EscPosEmulator](https://github.com/roydejong/EscPosEmulator).

Linux serial, mobile, HTML rendering, and Windows spooler jobs are not supported yet.
The minimum supported Rust version is 1.88, inherited from `escpos` 0.19.

## Install from source

Add the Rust plugin to the Tauri application:

```toml
[dependencies]
tauri-plugin-escpos = { git = "https://github.com/FrancoJFerreyra/tauri-plugin-escpos" }
```

Register it:

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_escpos::init())
    .run(tauri::generate_context!())
    .expect("error while running Tauri application");
```

Add the plugin permission to a capability:

```json
{
  "permissions": ["escpos:default"]
}
```

The JavaScript bindings live in `guest-js`. The crate ships the compiled `dist/` output, not the
TypeScript sources. Build them before packaging:

```bash
cd guest-js
npm install
npm run build
```

Then install from a local checkout or the published npm package:

```bash
npm install /path/to/tauri-plugin-escpos/guest-js
```

## Example

```ts
import { listPrinters, print, type PrintDocument } from 'tauri-plugin-escpos-api'

const [printer] = await listPrinters()
if (!printer) throw new Error('No Windows USB printer found')

const document: PrintDocument = {
  paperWidthMm: 80,
  characterSet: 'PC858',
  blocks: [
    {
      type: 'text',
      value: 'MY STORE',
      style: { align: 'center', bold: true, size: 'double' },
    },
    { type: 'divider' },
    {
      type: 'columns',
      columns: [
        { text: 'Coffee' },
        { text: '2', width: 0.15, align: 'right' },
        { text: '4.50 EUR', width: 0.3, align: 'right' },
      ],
    },
    {
      type: 'barcode',
      value: '123456789012',
      symbology: 'ean13',
      printValue: true,
    },
    { type: 'feed', lines: 2 },
    { type: 'cut' },
  ],
}

await print({
  printer: { kind: 'windows_usb', path: printer.path },
  document,
})
```

`cut` is explicit. The plugin initializes the printer and flushes the document, but it never adds a
cut command automatically.

## Blocks

- `text`: wrapped text with alignment, bold, underline, and normal/wide/tall/double size.
- `columns`: fixed-width receipt rows. The first column defaults to left alignment and the others
  to right alignment.
- `divider`: repeats one character across the paper width.
- `image`: base64 or data URL PNG/JPEG raster image.
- `barcode`: EAN-13, EAN-8, UPC-A, or CODE39.
- `qr`: native ESC/POS QR code.
- `feed`: advances paper by a number of lines.
- `cut`: full or partial cut.

Paper widths are `58` and `80` mm. Defaults are 80 mm and PC858.

## Printer selection

`listPrinters()` returns USB device interface paths from `usbprint.sys`. These are not Windows
spooler printer names and there is no default-printer policy. Persist the selected `id` or `path` in
the host application.

A printer can be addressed by USB path, USB identifiers, or TCP host and port:

```ts
{ kind: 'windows_usb', path: printer.path }
{ kind: 'windows_usb', vendorId: 0x04b8, productId: 0x0202 }
{ kind: 'network', host: '127.0.0.1', port: 9100 }
```

`listPrinters()` also returns a TCP target when:

- `ESCPOS_NETWORK_PRINTERS` lists `host:port` values (comma-separated), or
- a debug build is running (includes `127.0.0.1:9100` for local emulators), or
- a release build can connect to `127.0.0.1:9100`.

Use `selfTest()` to list printers and send the built-in test receipt to the first network printer
(or the first USB printer if none is listed). `testPrinter({ printer })` sends that same receipt to
a specific target.

## Errors

Rejected commands return an object with an English `message` and one of these stable codes:

- `unsupported_platform`
- `printer_not_found`
- `open_failed`
- `invalid_document`
- `print_failed`

Applications should translate user-facing messages from the code and retain `message` for logs.

## Development

```bash
cargo test --no-default-features
cd guest-js
npm install
npm run check
npm run build
```

Run `npm run build` before `cargo package` or `cargo publish` so `guest-js/dist` is present.

The no-default-features test command validates the platform-independent document renderer without
linking the Linux Tauri webview dependencies.

## License

MIT
