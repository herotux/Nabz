# Nabz architecture: one product, two interchangeable UIs

Nabz is one accounting product with two independently buildable frontends. It is not
two separate accounting applications.

## Components

- `crates/nabz-core`: canonical SQLite schema and migrations, database identity,
  safe frontend launcher, and complete SQLite backup/restore primitives.
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
`PRAGMA integrity_check`, schema version validation, and `foreign_key_check`.
Tauri asks for confirmation; Slint requires a second click before restoring the
latest automatic backup.

These controls restore the latest automatically generated snapshot. A user-facing
file picker for importing an arbitrary backup from another computer is a separate
feature and is not claimed here.

## Independent distribution

- Tauri CI builds its AppImage and Debian packages.
- Slint CI builds the native executable and packages a standalone amd64 Debian
  installer. Both artifacts are uploaded separately.
- Removing either frontend must not remove the shared data directory.
- The Slint Debian package currently targets amd64; Raspberry Pi and other
  architectures need their own matching build artifacts.

## Remaining architectural work

The database, schema versioning, launch handoff, and complete backup/restore are
shared. However, the accounting command implementations are not yet fully extracted:
Tauri and Slint still contain duplicated business operations and query logic. That
work is required before calling the entire business layer a single shared service.

CI is the compile/package verification gate. A successful GitHub commit or a queued
workflow is not proof of a successful build. Manjaro installation and real-database
cross-frontend acceptance testing must also be completed before declaring a release
ready.
