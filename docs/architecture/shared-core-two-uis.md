# Nabz architecture: one product, two interchangeable UIs

Nabz is one accounting product with two independently buildable frontends. It is not
two separate accounting applications.

## Components

- `crates/nabz-core`: canonical SQLite schema and migrations, database identity,
  safe frontend launcher, SQLite backup/restore primitives, and shared accounting services.
- `tauri-web`: independently buildable Tauri frontend.
- `slint-native`: independently buildable native Slint frontend.

Both frontends use the same core crate and the same per-user database. Installing,
removing, or switching a frontend must never create or delete a second accounting
ledger.

## Database identity

The canonical Linux database path is:

```text
$XDG_DATA_HOME/com.herotux.nabz.webprototype/nabz.sqlite3
```

When `XDG_DATA_HOME` is not set:

```text
~/.local/share/com.herotux.nabz.webprototype/nabz.sqlite3
```

On Windows, `%APPDATA%/com.herotux.nabz.webprototype/nabz.sqlite3` is preferred to
match Tauri's Roaming AppData directory. On macOS the path is under
`~/Library/Application Support/com.herotux.nabz.webprototype/nabz.sqlite3`.
`NABZ_DB_PATH` remains an explicit override for portable deployments and tests.

The shared initializer checks `PRAGMA user_version` before any schema DDL. An older
frontend refuses a newer database without first modifying it. SQLite foreign keys
and WAL mode are enabled by the common initializer.

## Switching between UIs

- Tauri's Settings page offers a switch to the native Slint frontend.
- Slint's header offers a switch to Tauri.
- The target executable is searched beside the current executable first, then via
  `PATH`. The current UI exits only after the other executable starts successfully.
- The target frontend must already be installed. If it is missing, the current UI
  stays open and reports the launch error.
- The switch is a process handoff, not a data conversion. Both binaries open the
  same SQLite file. Users should not run two frontends concurrently for normal
  accounting work.

Expected Linux executable names are `nabz-tauri` and `nabz-slint`.

## Backup and restore

The core uses SQLite's online backup API rather than copying only the main database
file. This includes every table, index, and schema object, including sales returns
and refunds that were absent from the earlier JSON export format.

Automatic backups are written to the shared data directory's `backups/` folder as
timestamped `nabz-*.sqlite3` files. Before restore, the source is checked with
`PRAGMA integrity_check`, schema version validation, required Nabz tables, and
`foreign_key_check`. A separate verified pre-restore snapshot is created first;
if replacement or post-restore migration fails, the core attempts to restore that
snapshot. The safety snapshot is retained after a successful restore as well.
Tauri asks for confirmation; Slint requires a second click before restoring the
latest automatic backup.

These controls restore the latest automatically generated snapshot. A user-facing
file picker for importing an arbitrary backup from another computer is a separate
feature and is not claimed here.

## Inventory costing and profit history

Schema version 6 adds `products.average_cost` as a separate field from the latest purchase price (`buy_price`).
New purchase invoices update the perpetual moving weighted-average cost using
checked integer arithmetic. Invoice-level purchase discounts are allocated across
lines by proportional largest remainder before they enter the cost basis. Sales
snapshot that average into
`invoice_items.unit_cost`, so later purchases and product edits do not rewrite
the cost or profit of an older sale. Sale returns and sale-invoice voids reintroduce
stock at the original saved unit cost; purchase returns and purchase-invoice voids
remove quantity at the current moving-average basis. Manual quantity adjustments
preserve the average cost, and an empty stock balance resets it to zero.

A v5 database has no complete opening-stock cost layers. Migration v6 therefore
seeds the opening `average_cost` from the product's currently recorded
`buy_price` for products with stock. This is an explicit opening estimate,
not a reconstruction of unavailable historical purchase layers. Existing
`invoice_items.unit_cost` values are never rewritten. Tauri's JSON backup
includes `average_cost` and restores it; older JSON backups without that field
fall back to their recorded purchase price. Full SQLite backups preserve the column
automatically.

## Independent distribution

- Tauri CI builds its AppImage and Debian packages.
- Slint CI builds the native executable and packages a standalone amd64 Debian
  installer. Both artifacts are uploaded separately.
- Removing either frontend must not remove the shared data directory.
- The Slint Debian package currently targets amd64; Raspberry Pi and other
  architectures need their own matching build artifacts.

## Remaining architectural work

The database, schema versioning, launch handoff, and backup/restore are shared.
`crates/nabz-core/src/accounting.rs` owns common contact/product operations,
transactional invoice creation, receipt/payment allocation, sale/purchase returns,
invoice voiding, inventory adjustments and movements, expense recording, profit/loss,
and inventory valuation. Both Tauri and Slint call the same accounting services.
Some dashboard summaries, presentation logic, and archive/delete policies remain
in the UI crates; the shared core is not yet a claim that every UI workflow has
been fully migrated.

CI is the compile/package verification gate. A successful GitHub commit or a queued
workflow is not proof of a successful build. Manjaro installation and real-database
cross-frontend acceptance testing must also be completed before declaring a release
ready.
