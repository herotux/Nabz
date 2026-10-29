# Nabz — Slint native UI prototype

This directory starts the native Slint replacement UI from the `tauri-web-prototype` branch. The existing Tauri UI and the legacy egui application are left untouched.

## Run on Linux

Install the standard Rust toolchain and the Linux development packages required by Slint's Winit backend (XCB, XKB common, fontconfig, and OpenSSL development headers as needed by the selected backend).

```bash
cargo run --manifest-path slint-native/Cargo.toml
```

Release build:

```bash
cargo build --release --manifest-path slint-native/Cargo.toml
```

## Database

The app reads the same SQLite filename used by the Tauri prototype: `nabz.sqlite3` under the app-local data directory `com.herotux.nabz.webprototype`. To select a database explicitly, set `NABZ_DB_PATH=/absolute/path/to/nabz.sqlite3`.

The UI currently contains the native dashboard shell and reads the four summary values from SQLite. Navigation entries are scaffolding only; contact/product/invoice/payment CRUD and printing are not yet connected to this UI. The Tauri implementation remains available on its original branch while this migration is developed and tested.

## RTL and Persian text

Text is authored in Persian and aligned to the right. Validate glyph shaping and keyboard entry on the target Manjaro machine before treating the native UI as ready for daily accounting.
