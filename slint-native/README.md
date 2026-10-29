# Nabz — Slint native UI migration

This branch is based on `tauri-web-prototype`. The Tauri implementation and the legacy egui app remain unchanged while the native Slint frontend is migrated and tested.

## Run on Linux

Install Rust stable and the development packages required by Slint/Winit. On Debian/Ubuntu CI these include `libxcb-shape0-dev`, `libxcb-xfixes0-dev`, `libxkbcommon-dev`, `libfontconfig1-dev`, `libssl-dev`, and `pkg-config`.

```bash
cargo run --manifest-path slint-native/Cargo.toml
```

Release binary:

```bash
cargo build --release --manifest-path slint-native/Cargo.toml
./slint-native/target/release/nabz-slint
```

## Database compatibility

By default the app uses the Tauri prototype's SQLite location:
`$XDG_DATA_HOME/com.herotux.nabz.webprototype/nabz.sqlite3`, falling back to `$HOME/.local/share/com.herotux.nabz.webprototype/nabz.sqlite3`. Set `NABZ_DB_PATH=/absolute/path/to/nabz.sqlite3` to override it.

**Important:** close the Tauri app before testing against the same database. Keep a backup before testing with real accounting data.

## Implemented in this migration step

- Native Slint window and Persian RTL-aligned dashboard.
- Live summary values read from SQLite.
- Contact registration and list (name, role, phone, opening balance).
- Product registration and list (code, barcode, stock, buy/sell price, minimum stock).
- Single-line sale/purchase invoices with transactional stock changes and receipt creation for paid sales.
- Receipt/payment entry with customer/supplier role checks, opening-balance reduction, and FIFO allocation to open invoices.
- Live invoice and payment lists.
- Store-name/currency settings persisted in SQLite.
- Persian and Arabic numeral parsing for amounts, validation messages, and basic unit tests.

## Not yet migrated / not yet accepted for production

- Editing/deleting records from the native UI.
- Multi-line invoices, returns, prepayments, advanced cost accounting, barcode scanner integration, label printing, A4/A5/thermal printer testing.
- Backup/restore UI and complete settings coverage.
- Verified keyboard navigation, Jalali date entry, font/glyph shaping and RTL behavior on Manjaro.
- Real-device performance measurements and Raspberry Pi smoke testing.

The Slint CI workflow checks compilation and uploads a Linux x86_64 binary artifact. A green CI run does not replace testing on the target Manjaro laptop.
