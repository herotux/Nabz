# Nabz — Slint native UI migration

This branch is based on `tauri-web-prototype`. The Tauri implementation and legacy egui app remain unchanged during this migration. The native application is still a prototype and must not be treated as production-ready until the remaining accounting, backup, printing, and device-validation work is complete.

## Build and run

Install Rust stable and the Linux development packages required by Slint/Winit. On Debian/Ubuntu CI these currently include:

```bash
sudo apt-get install -y libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libfontconfig1-dev libssl-dev pkg-config
```

Run from the repository root:

```bash
cargo run --manifest-path slint-native/Cargo.toml
```

Build the release binary:

```bash
cargo build --release --manifest-path slint-native/Cargo.toml
./slint-native/target/release/nabz-slint
```

## Database location and safety

By default the app opens the Tauri prototype's SQLite location:

- `$XDG_DATA_HOME/com.herotux.nabz.webprototype/nabz.sqlite3`
- or `$HOME/.local/share/com.herotux.nabz.webprototype/nabz.sqlite3`

Override it with `NABZ_DB_PATH=/absolute/path/to/nabz.sqlite3`.

**Important:** the native app currently creates/ensures its own tables and adds the invoice void flag when opening the database. A complete, versioned migration system and verified compatibility against a copy of a real Tauri database are still outstanding. Do not point this prototype at the only copy of a live accounting database. Close the Tauri app before testing against a database copy, and keep an independent backup.

For an isolated smoke test:

```bash
mkdir -p /tmp/nabz-slint-test
NABZ_DB_PATH=/tmp/nabz-slint-test/nabz.sqlite3 cargo run --manifest-path slint-native/Cargo.toml
```

## Implemented in the native prototype

- Native Slint window, Persian dashboard, and section navigation.
- Dashboard summaries and record lists backed by SQLite.
- Contact and product creation/listing, including role, opening balance, barcode field, stock, and prices.
- Multi-row sale and purchase invoices with transactional stock updates, discount, paid amount, and rollback on invalid rows or insufficient stock.
- Receipt/payment registration with contact-role checks, opening-balance reduction, and FIFO allocation to outstanding invoices.
- Invoice details, search by number/contact/notes, kind and void-status filters.
- Invoice voiding with stock reversal and restrictions after later allocations or returns.
- Partial sales returns with stock restoration, return history, proportional invoice-discount adjustment, over-return protection, and refund tracking.
- Store-name and currency settings persisted in SQLite.
- Persian/Arabic digit parsing and a growing set of accounting unit tests.

## Known gaps — not production-ready

- Versioned SQLite migrations and compatibility tests against a sanitized copy of an actual Tauri database.
- Complete accounting reconciliation and edge-case coverage, including multi-stage returns and date-sensitive refund reporting.
- Search/filter for contacts, products, and payments; safe edit/delete workflows.
- Barcode generation, USB HID scanner workflow, and label printing.
- Backup/restore with integrity validation and atomic replacement.
- A4/A5 and thermal invoice printing.
- Complete store profile, numbering, currency, and Jalali date settings.
- Verified Persian glyph shaping, true RTL behavior, keyboard navigation, and accessibility on Manjaro.
- Linux AppImage and Debian packages; the current Linux CI artifact is a raw x86_64 executable.
- Real-device performance measurements and Raspberry Pi smoke testing.

## CI

The Slint workflow checks the Linux build and uploads a raw x86_64 binary. The Windows workflow builds a Windows executable, portable archive, and installer. Green CI confirms only the checks actually run; it does not replace testing on Manjaro, physical printers, barcode scanners, or Raspberry Pi hardware.

See [TODO.md](TODO.md) for the migration checklist and release gates.
